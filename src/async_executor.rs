//! Integration with the `async-executor` crate.
//!
//! This module provides implementations of the [`Executor`] and [`LocalExecutor`] traits
//! for the `async-executor` crate, along with the [`AsyncTask`] wrapper.

use crate::{Executor, LocalExecutor, Task};
use core::{future::Future, mem::ManuallyDrop, pin::pin, task::Poll};

pub use async_executor::{Executor as AsyncExecutor, LocalExecutor as AsyncLocalExecutor};

#[cfg(feature = "std")]
use crate::catch_unwind;

#[cfg(not(feature = "std"))]
fn catch_unwind<F, R>(f: F) -> Result<R, crate::Error>
where
    F: FnOnce() -> R,
{
    // In no-std environments (like WASM), we can't catch panics
    // so we just execute the function directly
    Ok(f())
}

/// A task wrapper for `async_task::Task` that implements the [`Task`] trait.
///
/// This provides panic safety and proper error handling for tasks spawned
/// with the `async-executor` crate.
pub struct AsyncTask<T>(ManuallyDrop<Option<async_task::Task<T>>>);

impl<T> core::fmt::Debug for AsyncTask<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AsyncTask").finish_non_exhaustive()
    }
}

impl<T> From<async_task::Task<T>> for AsyncTask<T> {
    fn from(task: async_task::Task<T>) -> Self {
        Self(ManuallyDrop::new(Some(task)))
    }
}

impl<T> Future for AsyncTask<T> {
    type Output = T;

    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        self.as_mut()
            .poll_result(cx)
            .map(|res| res.expect("Task panicked"))
    }
}

impl<T> Task<T> for AsyncTask<T> {
    fn poll_result(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Result<T, crate::Error>> {
        let mut this = self.as_mut();

        let task = this.0.as_mut().expect("Task has already been cancelled");
        let result = catch_unwind(|| pin!(task).poll(cx));

        match result {
            Ok(Poll::Ready(value)) => Poll::Ready(Ok(value)),
            Ok(Poll::Pending) => Poll::Pending,
            Err(error) => Poll::Ready(Err(error)),
        }
    }
    fn poll_cancel(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<()> {
        let task = self.0.take().expect("Task has already been cancelled");
        let cancel_fut = task.cancel();
        pin!(cancel_fut).poll(cx).map(|_| {})
    }
}

impl Executor for async_executor::Executor<'static> {
    type Task<T: Send + 'static> = AsyncTask<T>;

    fn spawn<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future<Output: Send> + Send + 'static,
    {
        async_executor::Executor::spawn(self, fut).into()
    }
}

impl LocalExecutor for async_executor::LocalExecutor<'static> {
    type Task<T: 'static> = AsyncTask<T>;

    fn spawn<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future + 'static,
    {
        async_executor::LocalExecutor::spawn(self, fut).into()
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "std")]
    extern crate std;
    
    use super::*;
    use crate::{Executor, LocalExecutor, Task};
    use core::{pin::Pin, task::{Context, Poll, Waker}};
    use core::future::Future;
    use alloc::{sync::Arc, format};
    use alloc::task::Wake;

    struct TestWaker;
    impl Wake for TestWaker {
        fn wake(self: Arc<Self>) {}
    }

    fn create_waker() -> Waker {
        Arc::new(TestWaker).into()
    }

    async fn sleep_ms(ms: u64) {
        #[cfg(feature = "std")]
        {
            use std::time::{Duration, Instant};
            let start = Instant::now();
            while start.elapsed() < Duration::from_millis(ms) {
                futures_lite::future::yield_now().await;
                if start.elapsed() >= Duration::from_millis(ms) {
                    break;
                }
            }
        }
        #[cfg(not(feature = "std"))]
        {
            for _ in 0..ms {
                futures_lite::future::yield_now().await;
            }
        }
    }

    #[test]
    fn test_async_executor_spawn() {
        let ex = async_executor::Executor::new();
        let task: AsyncTask<i32> = Executor::spawn(&ex, async { 42 });
        
        let result = futures_lite::future::block_on(ex.run(task));
        assert_eq!(result, 42);
    }

    #[test]
    fn test_async_executor_spawn_async_operation() {
        let ex = async_executor::Executor::new();
        let task: AsyncTask<&str> = Executor::spawn(&ex, async {
            sleep_ms(1).await;
            "completed"
        });
        
        let result = futures_lite::future::block_on(ex.run(task));
        assert_eq!(result, "completed");
    }

    #[test]
    fn test_async_task_future_impl() {
        let ex = async_executor::Executor::new();
        let mut task: AsyncTask<i32> = Executor::spawn(&ex, async { 100 });
        
        let waker = create_waker();
        let mut cx = Context::from_waker(&waker);
        
        match Pin::new(&mut task).poll(&mut cx) {
            Poll::Ready(result) => assert_eq!(result, 100),
            Poll::Pending => {
                let result = futures_lite::future::block_on(ex.run(task));
                assert_eq!(result, 100);
            }
        }
    }

    #[test]
    fn test_async_task_poll_result() {
        let ex = async_executor::Executor::new();
        let mut task: AsyncTask<&str> = Executor::spawn(&ex, async { "success" });
        
        let waker = create_waker();
        let mut cx = Context::from_waker(&waker);
        
        match Pin::new(&mut task).poll_result(&mut cx) {
            Poll::Ready(Ok(result)) => assert_eq!(result, "success"),
            Poll::Ready(Err(_)) => panic!("Task should not fail"),
            Poll::Pending => {
                let result = futures_lite::future::block_on(ex.run(task.result()));
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), "success");
            }
        }
    }


    #[test]
    fn test_async_task_panic_handling() {
        let ex = async_executor::Executor::new();
        let task: AsyncTask<()> = Executor::spawn(&ex, async {
            panic!("test panic");
        });
        
        let result = futures_lite::future::block_on(ex.run(task.result()));
        assert!(result.is_err());
    }

    #[test]
    fn test_async_task_from_impl() {
        let ex = async_executor::Executor::new();
        let async_task = async_executor::Executor::spawn(&ex, async { 42 });
        let wrapped_task: AsyncTask<i32> = async_task.into();
        
        let result = futures_lite::future::block_on(ex.run(wrapped_task));
        assert_eq!(result, 42);
    }

    #[test]
    fn test_local_executor_spawn() {
        let local_ex = async_executor::LocalExecutor::new();
        let task: AsyncTask<&str> = LocalExecutor::spawn(&local_ex, async { "local task" });
        
        let result = futures_lite::future::block_on(local_ex.run(task));
        assert_eq!(result, "local task");
    }

    #[test]
    fn test_local_executor_spawn_non_send() {
        use alloc::rc::Rc;
        
        let local_ex = async_executor::LocalExecutor::new();
        let non_send_data = Rc::new(42);
        
        let task: AsyncTask<i32> = LocalExecutor::spawn(&local_ex, async move {
            *non_send_data
        });
        
        let result = futures_lite::future::block_on(local_ex.run(task));
        assert_eq!(result, 42);
    }

    #[test]
    fn test_async_task_poll_result_local() {
        let local_ex = async_executor::LocalExecutor::new();
        let mut task: AsyncTask<&str> = LocalExecutor::spawn(&local_ex, async { "local success" });
        
        let waker = create_waker();
        let mut cx = Context::from_waker(&waker);
        
        match Pin::new(&mut task).poll_result(&mut cx) {
            Poll::Ready(Ok(result)) => assert_eq!(result, "local success"),
            Poll::Ready(Err(_)) => panic!("Local task should not fail"),
            Poll::Pending => {
                let result = futures_lite::future::block_on(local_ex.run(task.result()));
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), "local success");
            }
        }
    }


    #[test]
    fn test_async_task_panic_handling_local() {
        let local_ex = async_executor::LocalExecutor::new();
        let task: AsyncTask<()> = LocalExecutor::spawn(&local_ex, async {
            panic!("local panic");
        });
        
        let result = futures_lite::future::block_on(local_ex.run(task.result()));
        assert!(result.is_err());
    }

    #[test]
    fn test_async_task_debug() {
        let ex = async_executor::Executor::new();
        let task: AsyncTask<i32> = Executor::spawn(&ex, async { 42 });
        let debug_str = format!("{:?}", task);
        assert!(debug_str.contains("AsyncTask"));
    }

    #[test]
    fn test_async_task_result_future() {
        let ex = async_executor::Executor::new();
        let task: AsyncTask<i32> = Executor::spawn(&ex, async { 123 });
        
        let result = futures_lite::future::block_on(ex.run(task.result()));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 123);
    }


    #[test]
    fn test_multiple_tasks_concurrency() {
        let ex = async_executor::Executor::new();
        
        let task1: AsyncTask<i32> = Executor::spawn(&ex, async {
            sleep_ms(10).await;
            1
        });
        
        let task2: AsyncTask<i32> = Executor::spawn(&ex, async {
            sleep_ms(5).await;
            2
        });
        
        let task3: AsyncTask<i32> = Executor::spawn(&ex, async { 3 });
        
        let result = futures_lite::future::block_on(ex.run(async {
            let r1 = task1.await;
            let r2 = task2.await;
            let r3 = task3.await;
            (r1, r2, r3)
        }));
        
        assert_eq!(result, (1, 2, 3));
    }

    #[test]
    fn test_async_task_manually_drop_safety() {
        let ex = async_executor::Executor::new();
        let mut task: AsyncTask<i32> = Executor::spawn(&ex, async { 42 });
        
        let waker = create_waker();
        let mut cx = Context::from_waker(&waker);
        
        let _poll_result = Pin::new(&mut task).poll_result(&mut cx);
        
        #[allow(clippy::drop_non_drop)]
        drop(task);
    }

    #[test] 
    fn test_catch_unwind_no_std() {
        use super::catch_unwind;
        
        let result = catch_unwind(|| {
            42
        });
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }
}
