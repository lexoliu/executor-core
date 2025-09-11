//! Integration for Web/WASM environments.
//!
//! This module provides executor implementations for web browsers and other
//! WASM environments using `wasm-bindgen-futures`.

use crate::{Executor, LocalExecutor, Task};
use core::{
    future::Future,
    mem::ManuallyDrop,
    pin::Pin,
    task::{Context, Poll},
};
use wasm_bindgen_futures::spawn_local;

/// Web-based executor implementation for WASM targets.
///
/// This executor uses `wasm-bindgen-futures::spawn_local` to execute futures
/// in web environments. Both `Send` and non-`Send` futures are handled the same
/// way since web environments are single-threaded.
///
/// ## Panic Handling
///
/// Unlike other executors, the web executor cannot catch panics due to WASM limitations.
/// If a spawned task panics, the entire WASM module will terminate. This is a fundamental
/// limitation of the WASM environment and cannot be worked around.
///
#[derive(Clone, Copy, Debug)]
pub struct WebExecutor;

impl WebExecutor {
    /// Create a new [`WebExecutor`].
    pub fn new() -> Self {
        Self
    }
}

impl Default for WebExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Task wrapper for web/WASM environment.
///
/// This task type provides task management for web environments where
/// panic catching is not available. Unlike other task implementations,
/// panics cannot be caught and will terminate the entire WASM module.
pub struct WebTask<T> {
    inner: ManuallyDrop<Option<async_task::Task<T>>>,
}

impl<T> core::fmt::Debug for WebTask<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("WebTask").finish_non_exhaustive()
    }
}

impl<T> Future for WebTask<T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.as_mut();
        let task = this
            .inner
            .as_mut()
            .expect("Task has already been cancelled");
        // In web environments, we can't catch panics, so just poll directly
        let mut pinned_task = core::pin::pin!(task);
        pinned_task.as_mut().poll(cx)
    }
}

impl<T: 'static> Task<T> for WebTask<T> {
    fn poll_result(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<T, crate::Error>> {
        let mut this = self.as_mut();
        let task = this
            .inner
            .as_mut()
            .expect("Task has already been cancelled");
        // In web environments, we can't catch panics
        // If the task panics, the entire WASM module will terminate
        let mut pinned_task = core::pin::pin!(task);
        match pinned_task.as_mut().poll(cx) {
            Poll::Ready(value) => Poll::Ready(Ok(value)),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_cancel(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        // Cancel the underlying task
        let this = unsafe { self.get_unchecked_mut() };
        if let Some(task) = this.inner.take() {
            // Schedule the cancellation but don't wait for it
            wasm_bindgen_futures::spawn_local(async move {
                let _ = task.cancel().await;
            });
        }
        Poll::Ready(())
    }
}

impl LocalExecutor for WebExecutor {
    type Task<T: 'static> = WebTask<T>;

    fn spawn_local<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future + 'static,
    {
        let (runnable, task) = async_task::spawn_local(fut, |runnable: async_task::Runnable| {
            spawn_local(async move {
                runnable.run();
            });
        });
        runnable.schedule();
        WebTask {
            inner: ManuallyDrop::new(Some(task)),
        }
    }
}

impl Executor for WebExecutor {
    type Task<T: Send + 'static> = WebTask<T>;

    fn spawn<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future<Output: Send> + Send + 'static,
    {
        // In web environment, we use spawn_local even for Send futures
        // since web workers don't have the same threading model as native
        let (runnable, task) = async_task::spawn_local(fut, |runnable: async_task::Runnable| {
            spawn_local(async move {
                runnable.run();
            });
        });
        runnable.schedule();
        WebTask {
            inner: ManuallyDrop::new(Some(task)),
        }
    }
}
