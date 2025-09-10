#[cfg(feature = "std")]
extern crate std;

use crate::{Executor, LocalExecutor, Task};
use alloc::boxed::Box;
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

/// Tokio-based executor implementation
#[derive(Clone)]
pub struct DefaultExecutor;

impl DefaultExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Task wrapper for tokio JoinHandle
pub struct TokioTask<T> {
    handle: tokio::task::JoinHandle<T>,
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

impl Executor for DefaultExecutor {
    type Task<T: Send + 'static> = TokioTask<T>;

    fn spawn<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future<Output: Send> + Send + 'static,
    {
        let handle = tokio::task::spawn(fut);
        TokioTask { handle }
    }
}

/// Task wrapper for tokio local JoinHandle (non-Send futures)
pub struct TokioLocalTask<T> {
    handle: tokio::task::JoinHandle<T>,
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

impl LocalExecutor for DefaultExecutor {
    type Task<T: 'static> = TokioLocalTask<T>;

    fn spawn<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
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

    fn spawn<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future + 'static,
    {
        let handle = self.spawn_local(fut);
        TokioLocalTask { handle }
    }
}
