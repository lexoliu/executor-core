//! Integration with the `async-executor` crate.
//!
//! This module provides implementations of the [`Executor`] and [`LocalExecutor`] traits
//! for the `async-executor` crate, along with the [`AsyncTask`] wrapper.

use crate::{Executor, LocalExecutor, Task};
use core::{future::Future, mem::ManuallyDrop, pin::pin, task::Poll};

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
