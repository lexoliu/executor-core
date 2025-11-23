//! Integration with the `async-task` crate.
//!
//! This module provides a unified wrapper around the `async-task` crate that can be used
//! by different executor implementations. It offers task spawning utilities and a
//! task wrapper that implements the [`Task`] trait.

use crate::{Error, Task};
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

pub use async_task::{Runnable, Task as RawTask};

#[cfg(feature = "std")]
use crate::catch_unwind;

#[cfg(not(feature = "std"))]
fn catch_unwind<F, R>(f: F) -> Result<R, Error>
where
    F: FnOnce() -> R,
{
    // In no-std environments (like WASM), we can't catch panics
    // so we just execute the function directly
    Ok(f())
}

/// A task wrapper that implements the [`Task`] trait.
///
/// This provides panic safety and proper error handling for tasks created
/// with the `async-task` crate.
pub struct AsyncTask<T>(async_task::Task<T>);

impl<T> core::fmt::Debug for AsyncTask<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AsyncTask").finish_non_exhaustive()
    }
}

impl<T> From<async_task::Task<T>> for AsyncTask<T> {
    fn from(task: async_task::Task<T>) -> Self {
        Self(task)
    }
}

impl<T> Future for AsyncTask<T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

impl<T> Task<T> for AsyncTask<T> {
    fn poll_result(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<T, Error>> {
        match Pin::new(&mut self.0).poll(cx) {
            Poll::Ready(value) => {
                #[cfg(feature = "std")]
                {
                    // In std environments, we catch panics to return as errors
                    match catch_unwind(|| value) {
                        Ok(v) => Poll::Ready(Ok(v)),
                        Err(error) => Poll::Ready(Err(error)),
                    }
                }
                #[cfg(not(feature = "std"))]
                {
                    // In no-std environments, we can't catch panics
                    Poll::Ready(Ok(value))
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Spawn a future with a custom scheduler using `async_task`.
///
/// This function creates a task that will be scheduled using the provided scheduler function.
/// The scheduler receives a [`Runnable`] that should be executed to make progress on the task.
///
/// Returns a tuple of (runnable, task) where:
/// - `runnable` should be scheduled immediately to start the task
/// - `task` is an [`AsyncTask`] that can be awaited for the result
pub fn spawn<F, S>(future: F, scheduler: S) -> (Runnable, AsyncTask<F::Output>)
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
    S: Fn(Runnable) + Send + Sync + 'static,
{
    let (runnable, task) = async_task::spawn(future, scheduler);
    (runnable, AsyncTask::from(task))
}

/// Spawn a local (non-Send) future with a custom scheduler using `async_task`.
///
/// This is similar to [`spawn`] but works with futures that are not `Send`.
/// It uses `async_task::spawn_local` internally.
///
/// This function is only available when the `std` feature is enabled.
#[cfg(feature = "std")]
pub fn spawn_local<F, S>(future: F, scheduler: S) -> (Runnable, AsyncTask<F::Output>)
where
    F: Future + 'static,
    S: Fn(Runnable) + Send + Sync + 'static,
{
    let (runnable, task) = async_task::spawn_local(future, scheduler);
    (runnable, AsyncTask::from(task))
}
