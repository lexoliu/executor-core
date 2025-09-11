//! Integration with the Tokio async runtime.
//!
//! This module provides implementations of the [`Executor`] and [`LocalExecutor`] traits
//! for the Tokio runtime, along with task wrappers that provide panic safety.

#[cfg(feature = "std")]
extern crate std;

use crate::{Executor, LocalExecutor, Task};
use alloc::boxed::Box;
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

/// The default Tokio-based executor implementation.
///
/// This executor can spawn both Send and non-Send futures using Tokio's
/// `spawn` and `spawn_local` functions respectively.
///
#[derive(Clone, Copy, Debug)]
pub struct TokioExecutor;

pub use tokio::{runtime::Runtime, task::JoinHandle, task::LocalSet};

impl TokioExecutor {
    /// Create a new [`TokioExecutor`].
    pub fn new() -> Self {
        Self
    }
}

impl Default for TokioExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Task wrapper for Tokio's `JoinHandle` that implements the [`Task`] trait.
///
/// This provides panic safety and proper error handling for tasks spawned
/// with Tokio's `spawn` function.
pub struct TokioTask<T> {
    handle: tokio::task::JoinHandle<T>,
}

impl<T> core::fmt::Debug for TokioTask<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TokioTask").finish_non_exhaustive()
    }
}

impl<T: Send + 'static> Future for TokioTask<T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.handle).poll(cx) {
            Poll::Ready(Ok(result)) => Poll::Ready(result),
            Poll::Ready(Err(err)) => {
                if err.is_panic() {
                    std::panic::resume_unwind(err.into_panic());
                } else {
                    // Task was cancelled
                    std::panic::panic_any("Task was cancelled")
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<T: Send + 'static> Task<T> for TokioTask<T> {
    fn poll_result(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<T, crate::Error>> {
        match Pin::new(&mut self.handle).poll(cx) {
            Poll::Ready(Ok(result)) => Poll::Ready(Ok(result)),
            Poll::Ready(Err(err)) => {
                let error: crate::Error = if err.is_panic() {
                    err.into_panic()
                } else {
                    Box::new("Task was cancelled")
                };
                Poll::Ready(Err(error))
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_cancel(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        let this = unsafe { self.get_unchecked_mut() };
        this.handle.abort();
        Poll::Ready(())
    }
}

impl Executor for TokioExecutor {
    type Task<T: Send + 'static> = TokioTask<T>;

    fn spawn<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future<Output: Send> + Send + 'static,
    {
        let handle = tokio::task::spawn(fut);
        TokioTask { handle }
    }
}

/// Task wrapper for Tokio's local `JoinHandle` (non-Send futures).
///
/// This provides panic safety and proper error handling for tasks spawned
/// with Tokio's `spawn_local` function.
pub struct TokioLocalTask<T> {
    handle: tokio::task::JoinHandle<T>,
}

impl<T> core::fmt::Debug for TokioLocalTask<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TokioLocalTask").finish_non_exhaustive()
    }
}

impl<T: 'static> Future for TokioLocalTask<T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.handle).poll(cx) {
            Poll::Ready(Ok(result)) => Poll::Ready(result),
            Poll::Ready(Err(err)) => {
                if err.is_panic() {
                    std::panic::resume_unwind(err.into_panic());
                } else {
                    // Task was cancelled
                    std::panic::panic_any("Task was cancelled")
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<T: 'static> Task<T> for TokioLocalTask<T> {
    fn poll_result(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<T, crate::Error>> {
        match Pin::new(&mut self.handle).poll(cx) {
            Poll::Ready(Ok(result)) => Poll::Ready(Ok(result)),
            Poll::Ready(Err(err)) => {
                let error: crate::Error = if err.is_panic() {
                    err.into_panic()
                } else {
                    Box::new("Task was cancelled")
                };
                Poll::Ready(Err(error))
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_cancel(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        let this = unsafe { self.get_unchecked_mut() };
        this.handle.abort();
        Poll::Ready(())
    }
}

impl LocalExecutor for TokioExecutor {
    type Task<T: 'static> = TokioLocalTask<T>;

    fn spawn_local<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future + 'static,
    {
        let handle = tokio::task::spawn_local(fut);
        TokioLocalTask { handle }
    }
}

impl Executor for tokio::runtime::Runtime {
    type Task<T: Send + 'static> = TokioTask<T>;

    fn spawn<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future<Output: Send> + Send + 'static,
    {
        let handle = self.spawn(fut);
        TokioTask { handle }
    }
}

impl LocalExecutor for tokio::task::LocalSet {
    type Task<T: 'static> = TokioLocalTask<T>;

    fn spawn_local<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future + 'static,
    {
        let handle = self.spawn_local(fut);
        TokioLocalTask { handle }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Executor, LocalExecutor, Task};
    use alloc::task::Wake;
    use alloc::{format, sync::Arc};
    use core::future::Future;
    use core::{
        pin::Pin,
        task::{Context, Poll, Waker},
    };
    use tokio::time::{Duration, sleep};

    struct TestWaker;
    impl Wake for TestWaker {
        fn wake(self: Arc<Self>) {}
    }

    fn create_waker() -> Waker {
        Arc::new(TestWaker).into()
    }

    #[tokio::test]
    async fn test_default_executor_spawn() {
        let executor = TokioExecutor::new();
        let task: TokioTask<i32> = Executor::spawn(&executor, async { 42 });
        let result = task.await;
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_default_executor_spawn_async_operation() {
        let executor = TokioExecutor::new();
        let task: TokioTask<&str> = Executor::spawn(&executor, async {
            sleep(Duration::from_millis(10)).await;
            "completed"
        });
        let result = task.await;
        assert_eq!(result, "completed");
    }

    #[tokio::test]
    async fn test_tokio_task_future_impl() {
        let executor = TokioExecutor::new();
        let mut task: TokioTask<i32> = Executor::spawn(&executor, async { 100 });

        let waker = create_waker();
        let mut cx = Context::from_waker(&waker);

        match Pin::new(&mut task).poll(&mut cx) {
            Poll::Ready(result) => assert_eq!(result, 100),
            Poll::Pending => {
                let result = task.await;
                assert_eq!(result, 100);
            }
        }
    }

    #[tokio::test]
    async fn test_tokio_task_poll_result() {
        let executor = TokioExecutor::new();
        let mut task: TokioTask<&str> = Executor::spawn(&executor, async { "success" });

        let waker = create_waker();
        let mut cx = Context::from_waker(&waker);

        match Pin::new(&mut task).poll_result(&mut cx) {
            Poll::Ready(Ok(result)) => assert_eq!(result, "success"),
            Poll::Ready(Err(_)) => panic!("Task should not fail"),
            Poll::Pending => {
                let result = task.result().await;
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), "success");
            }
        }
    }

    #[tokio::test]
    async fn test_tokio_task_cancel() {
        let executor = TokioExecutor::new();
        let mut task: TokioTask<&str> = Executor::spawn(&executor, async {
            sleep(Duration::from_secs(10)).await;
            "should be cancelled"
        });

        let waker = create_waker();
        let mut cx = Context::from_waker(&waker);

        let cancel_result = Pin::new(&mut task).poll_cancel(&mut cx);
        assert_eq!(cancel_result, Poll::Ready(()));
    }

    #[tokio::test]
    async fn test_tokio_task_panic_handling() {
        let executor = TokioExecutor::new();
        let task: TokioTask<()> = Executor::spawn(&executor, async {
            panic!("test panic");
        });

        let result = task.result().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_default_executor_default() {
        let executor1 = TokioExecutor::new();
        let executor2 = TokioExecutor::new();

        let task1: TokioTask<i32> = Executor::spawn(&executor1, async { 1 });
        let task2: TokioTask<i32> = Executor::spawn(&executor2, async { 2 });

        assert_eq!(task1.await, 1);
        assert_eq!(task2.await, 2);
    }

    #[test]
    fn test_runtime_executor_impl() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let task: TokioTask<&str> = Executor::spawn(&rt, async { "runtime task" });
        let result = rt.block_on(task);
        assert_eq!(result, "runtime task");
    }

    #[tokio::test]
    async fn test_local_set_executor() {
        let local_set = tokio::task::LocalSet::new();

        local_set
            .run_until(async {
                let task: TokioLocalTask<&str> =
                    LocalExecutor::spawn_local(&local_set, async { "local task" });
                let result = task.await;
                assert_eq!(result, "local task");
            })
            .await;
    }

    #[tokio::test]
    async fn test_tokio_local_task_future_impl() {
        let local_set = tokio::task::LocalSet::new();

        local_set
            .run_until(async {
                let mut task: TokioLocalTask<i32> =
                    LocalExecutor::spawn_local(&local_set, async { 200 });

                let waker = create_waker();
                let mut cx = Context::from_waker(&waker);

                match Pin::new(&mut task).poll(&mut cx) {
                    Poll::Ready(result) => assert_eq!(result, 200),
                    Poll::Pending => {
                        let result = task.await;
                        assert_eq!(result, 200);
                    }
                }
            })
            .await;
    }

    #[tokio::test]
    async fn test_tokio_local_task_poll_result() {
        let local_set = tokio::task::LocalSet::new();

        local_set
            .run_until(async {
                let mut task: TokioLocalTask<&str> =
                    LocalExecutor::spawn_local(&local_set, async { "local success" });

                let waker = create_waker();
                let mut cx = Context::from_waker(&waker);

                match Pin::new(&mut task).poll_result(&mut cx) {
                    Poll::Ready(Ok(result)) => assert_eq!(result, "local success"),
                    Poll::Ready(Err(_)) => panic!("Local task should not fail"),
                    Poll::Pending => {
                        let result = task.result().await;
                        assert!(result.is_ok());
                        assert_eq!(result.unwrap(), "local success");
                    }
                }
            })
            .await;
    }

    #[tokio::test]
    async fn test_tokio_local_task_cancel() {
        let local_set = tokio::task::LocalSet::new();

        local_set
            .run_until(async {
                let mut task: TokioLocalTask<&str> =
                    LocalExecutor::spawn_local(&local_set, async {
                        sleep(Duration::from_secs(10)).await;
                        "should be cancelled"
                    });

                let waker = create_waker();
                let mut cx = Context::from_waker(&waker);

                let cancel_result = Pin::new(&mut task).poll_cancel(&mut cx);
                assert_eq!(cancel_result, Poll::Ready(()));
            })
            .await;
    }

    #[tokio::test]
    async fn test_tokio_local_task_panic_handling() {
        let local_set = tokio::task::LocalSet::new();

        local_set
            .run_until(async {
                let task: TokioLocalTask<()> = LocalExecutor::spawn_local(&local_set, async {
                    panic!("local panic");
                });

                let result = task.result().await;
                assert!(result.is_err());
            })
            .await;
    }

    #[test]
    fn test_tokio_task_debug() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let task: TokioTask<i32> = Executor::spawn(&rt, async { 42 });
        let debug_str = format!("{:?}", task);
        assert!(debug_str.contains("TokioTask"));
    }

    #[test]
    fn test_tokio_local_task_debug() {
        let local_set = tokio::task::LocalSet::new();
        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(local_set.run_until(async {
            let task: TokioLocalTask<i32> = LocalExecutor::spawn_local(&local_set, async { 42 });
            let debug_str = format!("{:?}", task);
            assert!(debug_str.contains("TokioLocalTask"));
        }));
    }

    #[test]
    fn test_default_executor_debug() {
        let executor = TokioExecutor::new();
        let debug_str = format!("{:?}", executor);
        assert!(debug_str.contains("TokioExecutor"));
    }

    #[tokio::test]
    async fn test_task_result_future() {
        let executor = TokioExecutor::new();
        let task: TokioTask<i32> = Executor::spawn(&executor, async { 123 });

        let result = task.result().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 123);
    }

    #[tokio::test]
    async fn test_task_cancel_future() {
        let executor = TokioExecutor::new();
        let task: TokioTask<&str> = Executor::spawn(&executor, async {
            sleep(Duration::from_secs(10)).await;
            "cancelled"
        });

        task.cancel().await;
    }

    #[tokio::test]
    async fn test_multiple_tasks_concurrency() {
        let executor = TokioExecutor::new();

        let task1: TokioTask<i32> = Executor::spawn(&executor, async {
            sleep(Duration::from_millis(50)).await;
            1
        });

        let task2: TokioTask<i32> = Executor::spawn(&executor, async {
            sleep(Duration::from_millis(25)).await;
            2
        });

        let task3: TokioTask<i32> = Executor::spawn(&executor, async { 3 });

        let (r1, r2, r3) = tokio::join!(task1, task2, task3);
        assert_eq!(r1, 1);
        assert_eq!(r2, 2);
        assert_eq!(r3, 3);
    }
}
