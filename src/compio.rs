//! Integration with the compio async runtime.
//!
//! compio's [`Runtime`] is thread-local (`!Send`, `!Sync`), so the two traits map onto two
//! different compio types:
//!
//! - [`LocalExecutor`] is implemented for [`Runtime`] itself. Spawning goes straight to
//!   [`Runtime::spawn`], and the task is driven by whoever runs that runtime
//!   (for example [`Runtime::block_on`]).
//! - [`Executor`] (`Send` futures, callable from any thread) is implemented for
//!   [`Dispatcher`], compio's thread-per-core pool: each spawned future is sent to a worker
//!   thread, built there and driven by that thread's own compio runtime.
//!
//! Dropping a task cancels it on both sides, and [`Task::detach`] lets it run to completion.

extern crate std;

use crate::{Error, Executor, LocalExecutor, Task};
use alloc::boxed::Box;
use async_channel::{Receiver, Sender};
use core::{
    future::{Future, poll_fn},
    panic::AssertUnwindSafe,
    pin::{Pin, pin},
    task::{Context, Poll},
};
use std::panic::{catch_unwind, resume_unwind};

use compio::runtime::{JoinError, JoinHandle};
pub use compio::{dispatcher::Dispatcher, runtime::Runtime};

fn cancelled() -> Error {
    Box::new("Task was cancelled")
}

/// Task spawned on a compio [`Runtime`] through [`LocalExecutor`].
///
/// Dropping it cancels the task.
pub struct CompioLocalTask<T> {
    handle: JoinHandle<T>,
}

impl<T> core::fmt::Debug for CompioLocalTask<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CompioLocalTask").finish_non_exhaustive()
    }
}

impl<T> Future for CompioLocalTask<T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        Pin::new(&mut self.handle)
            .poll(cx)
            .map(|result| match result {
                Ok(value) => value,
                Err(JoinError::Panicked(payload)) => resume_unwind(payload),
                Err(JoinError::Cancelled) => std::panic::panic_any("Task was cancelled"),
            })
    }
}

impl<T> Task<T> for CompioLocalTask<T> {
    fn poll_result(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<T, Error>> {
        Pin::new(&mut self.handle)
            .poll(cx)
            .map(|result| match result {
                Ok(value) => Ok(value),
                Err(JoinError::Panicked(payload)) => Err(payload),
                Err(JoinError::Cancelled) => Err(cancelled()),
            })
    }

    fn detach(self) {
        self.handle.detach();
    }
}

impl LocalExecutor for Runtime {
    type Task<T: 'static> = CompioLocalTask<T>;

    fn spawn_local<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future + 'static,
    {
        CompioLocalTask {
            handle: self.spawn(fut),
        }
    }
}

/// A finished future's output, or its panic payload.
type Outcome<T> = Result<T, Error>;

/// Task spawned on a compio [`Dispatcher`] through [`Executor`].
///
/// Dropping it cancels the future on its worker thread.
pub struct CompioTask<T> {
    /// Resolves with the future's output or its panic payload, once the worker is done.
    result: Pin<Box<dyn Future<Output = Option<Outcome<T>>> + Send>>,
    /// Tells the worker to drop the future. `None` once detached.
    cancel: Option<Sender<()>>,
}

impl<T> core::fmt::Debug for CompioTask<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CompioTask").finish_non_exhaustive()
    }
}

impl<T> Future for CompioTask<T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        self.as_mut().poll_result(cx).map(|result| match result {
            Ok(value) => value,
            Err(payload) => resume_unwind(payload),
        })
    }
}

impl<T> Task<T> for CompioTask<T> {
    fn poll_result(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<T, Error>> {
        // The worker dropped its end without a result: it was shut down mid-flight.
        self.result
            .as_mut()
            .poll(cx)
            .map(|result| result.unwrap_or_else(|| Err(cancelled())))
    }

    fn detach(mut self) {
        // Closing the channel without a message means "keep running".
        drop(self.cancel.take());
    }
}

impl<T> Drop for CompioTask<T> {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            // The channel holds one message and is sent to at most once, so this cannot fail
            // for lack of room; a closed channel means the worker already finished.
            let _ = cancel.try_send(());
        }
    }
}

/// Runs `fut` to completion on the worker, catching its panic, until `cancel` carries a
/// message. A cancel channel that closes empty means the task was detached.
async fn drive<Fut: Future>(fut: Fut, cancel: Receiver<()>) -> Option<Result<Fut::Output, Error>> {
    let mut fut = pin!(fut);
    // Kept across polls: dropping it would drop the wake registration with it.
    let mut recv = pin!(cancel.recv());
    let mut detached = false;
    poll_fn(|cx| {
        if !detached {
            match recv.as_mut().poll(cx) {
                Poll::Ready(Ok(())) => return Poll::Ready(None),
                Poll::Ready(Err(_)) => detached = true,
                Poll::Pending => {}
            }
        }
        match catch_unwind(AssertUnwindSafe(|| fut.as_mut().poll(cx))) {
            Ok(Poll::Ready(value)) => Poll::Ready(Some(Ok(value))),
            Ok(Poll::Pending) => Poll::Pending,
            Err(payload) => Poll::Ready(Some(Err(payload))),
        }
    })
    .await
}

impl Executor for Dispatcher {
    type Task<T: Send + 'static> = CompioTask<T>;

    fn spawn<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future<Output: Send> + Send + 'static,
    {
        let (result_tx, result_rx) = async_channel::bounded(1);
        let (cancel_tx, cancel_rx) = async_channel::bounded(1);

        let dispatched = self.dispatch(move || async move {
            let outcome = drive(fut, cancel_rx).await;
            if let Some(outcome) = outcome {
                // The task may already be dropped; nobody is left to read the result.
                let _ = result_tx.send(outcome).await;
            }
        });
        if dispatched.is_err() {
            panic!("every compio dispatcher worker has panicked; cannot spawn");
        }

        CompioTask {
            result: Box::pin(async move { result_rx.recv().await.ok() }),
            cancel: Some(cancel_tx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{format, sync::Arc};
    use core::sync::atomic::{AtomicBool, Ordering};
    use std::{thread, time::Duration};

    /// Sets a flag when dropped, proving the future holding it was dropped.
    struct DropFlag(Arc<AtomicBool>);

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    fn dispatcher() -> Dispatcher {
        Dispatcher::new().expect("dispatcher")
    }

    fn wait_for(flag: &AtomicBool) {
        for _ in 0..500 {
            if flag.load(Ordering::SeqCst) {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("condition not reached");
    }

    #[test]
    fn spawn_returns_output() {
        let ex = dispatcher();
        let task: CompioTask<i32> = Executor::spawn(&ex, async { 42 });
        assert_eq!(futures_lite::future::block_on(task), 42);
    }

    #[test]
    fn spawn_from_other_thread() {
        let ex = Arc::new(dispatcher());
        let ex2 = ex.clone();
        let task = thread::spawn(move || Executor::spawn(&*ex2, async { "remote" }))
            .join()
            .unwrap();
        assert_eq!(futures_lite::future::block_on(task), "remote");
    }

    #[test]
    fn spawn_runs_off_the_calling_thread() {
        let ex = dispatcher();
        let caller = thread::current().id();
        let task = Executor::spawn(&ex, async move { thread::current().id() });
        assert_ne!(futures_lite::future::block_on(task), caller);
    }

    #[test]
    fn spawn_async_operation() {
        let ex = dispatcher();
        let task = Executor::spawn(&ex, async {
            for _ in 0..10 {
                futures_lite::future::yield_now().await;
            }
            "completed"
        });
        assert_eq!(futures_lite::future::block_on(task), "completed");
    }

    #[test]
    fn spawn_poll_result() {
        let ex = dispatcher();
        let task = Executor::spawn(&ex, async { "success" });
        let result = futures_lite::future::block_on(task.result());
        assert_eq!(result.unwrap(), "success");
    }

    #[test]
    fn spawn_panic_is_reported_and_worker_survives() {
        let ex = dispatcher();
        let task: CompioTask<()> = Executor::spawn(&ex, async { panic!("test panic") });
        let error = futures_lite::future::block_on(task.result()).unwrap_err();
        assert_eq!(*error.downcast::<&str>().unwrap(), "test panic");

        let task = Executor::spawn(&ex, async { 1 });
        assert_eq!(futures_lite::future::block_on(task), 1);
    }

    #[test]
    fn spawn_many_tasks_concurrently() {
        let ex = dispatcher();
        let tasks: alloc::vec::Vec<_> = (0..64)
            .map(|i| Executor::spawn(&ex, async move { i }))
            .collect();
        let sum: i32 = tasks.into_iter().map(futures_lite::future::block_on).sum();
        assert_eq!(sum, (0..64).sum());
    }

    #[test]
    fn dropping_a_task_cancels_it() {
        let ex = dispatcher();
        let started = Arc::new(AtomicBool::new(false));
        let dropped = Arc::new(AtomicBool::new(false));

        let task = {
            let (started, flag) = (started.clone(), DropFlag(dropped.clone()));
            Executor::spawn(&ex, async move {
                let _flag = flag;
                started.store(true, Ordering::SeqCst);
                core::future::pending::<()>().await;
            })
        };
        wait_for(&started);
        assert!(!dropped.load(Ordering::SeqCst));

        drop(task);
        wait_for(&dropped);
    }

    #[test]
    fn dropping_a_task_cancels_it_before_it_starts() {
        let ex = dispatcher();
        let dropped = Arc::new(AtomicBool::new(false));
        let flag = DropFlag(dropped.clone());
        let task = Executor::spawn(&ex, async move {
            let _flag = flag;
            core::future::pending::<()>().await;
        });
        drop(task);
        wait_for(&dropped);
    }

    #[test]
    fn detached_task_runs_to_completion() {
        let ex = dispatcher();
        let done = Arc::new(AtomicBool::new(false));
        let flag = done.clone();
        Executor::spawn(&ex, async move {
            for _ in 0..10 {
                futures_lite::future::yield_now().await;
            }
            flag.store(true, Ordering::SeqCst);
        })
        .detach();
        wait_for(&done);
    }

    #[test]
    fn task_debug() {
        let ex = dispatcher();
        let task = Executor::spawn(&ex, async { 1 });
        assert!(format!("{task:?}").contains("CompioTask"));
    }

    #[test]
    fn spawn_local_returns_output() {
        let rt = Runtime::new().expect("runtime");
        let task: CompioLocalTask<i32> = LocalExecutor::spawn_local(&rt, async { 42 });
        assert_eq!(rt.block_on(task), 42);
    }

    #[test]
    fn spawn_local_accepts_non_send_futures() {
        let rt = Runtime::new().expect("runtime");
        let value = alloc::rc::Rc::new(7);
        let task = LocalExecutor::spawn_local(&rt, async move { *value });
        assert_eq!(rt.block_on(task), 7);
    }

    #[test]
    fn spawn_local_poll_result() {
        let rt = Runtime::new().expect("runtime");
        let task = LocalExecutor::spawn_local(&rt, async { "local success" });
        assert_eq!(rt.block_on(task.result()).unwrap(), "local success");
    }

    #[test]
    fn spawn_local_panic_is_reported() {
        let rt = Runtime::new().expect("runtime");
        let task: CompioLocalTask<()> =
            LocalExecutor::spawn_local(&rt, async { panic!("local panic") });
        let error = rt.block_on(task.result()).unwrap_err();
        assert_eq!(*error.downcast::<&str>().unwrap(), "local panic");
    }

    #[test]
    fn spawn_local_dropping_a_task_cancels_it() {
        let rt = Runtime::new().expect("runtime");
        let dropped = Arc::new(AtomicBool::new(false));
        let flag = DropFlag(dropped.clone());
        rt.block_on(async {
            let task = LocalExecutor::spawn_local(&rt, async move {
                let _flag = flag;
                core::future::pending::<()>().await;
            });
            futures_lite::future::yield_now().await;
            assert!(!dropped.load(Ordering::SeqCst));
            drop(task);
            futures_lite::future::yield_now().await;
        });
        assert!(dropped.load(Ordering::SeqCst));
    }

    #[test]
    fn spawn_local_detached_task_runs_to_completion() {
        let rt = Runtime::new().expect("runtime");
        let done = Arc::new(AtomicBool::new(false));
        let flag = done.clone();
        rt.block_on(async {
            LocalExecutor::spawn_local(&rt, async move {
                flag.store(true, Ordering::SeqCst);
            })
            .detach();
            for _ in 0..10 {
                futures_lite::future::yield_now().await;
            }
        });
        assert!(done.load(Ordering::SeqCst));
    }

    #[test]
    fn spawn_local_task_debug() {
        let rt = Runtime::new().expect("runtime");
        let task = LocalExecutor::spawn_local(&rt, async { 1 });
        assert!(format!("{task:?}").contains("CompioLocalTask"));
    }
}
