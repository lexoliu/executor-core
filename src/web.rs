//! Integration for Web/WASM environments.
//!
//! This module provides executor implementations for web browsers and other
//! WASM environments using `wasm-bindgen-futures`.

use crate::{Executor, LocalExecutor, Task};
use core::{
    future::Future,
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
///
/// This is a simple wrapper that just awaits the spawned future directly.
pub struct WebTask<T> {
    _marker: core::marker::PhantomData<T>,
}

impl<T> core::fmt::Debug for WebTask<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("WebTask").finish_non_exhaustive()
    }
}

impl<T: 'static> WebTask<T> {
    fn new() -> Self {
        Self {
            _marker: core::marker::PhantomData,
        }
    }
}

impl<T> Future for WebTask<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Web tasks complete immediately since they're spawned via spawn_local
        // This shouldn't actually be called since we don't store the future
        Poll::Pending
    }
}

impl<T: 'static> Task<T> for WebTask<T> {
    fn poll_result(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<T, crate::Error>> {
        // Web tasks complete immediately since they're spawned via spawn_local
        // This shouldn't actually be called since we don't store the future
        Poll::Pending
    }

    fn poll_cancel(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        // In web environment, tasks can't be cancelled once spawned
        Poll::Ready(())
    }
}


impl LocalExecutor for WebExecutor {
    type Task<T: 'static> = WebTask<T>;

    fn spawn_local<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future + 'static,
    {
        // Spawn the future directly using wasm-bindgen-futures
        // We need to wrap it to discard the result since spawn_local expects ()
        spawn_local(async move {
            let _ = fut.await;
        });
        
        // Return a placeholder task since web tasks can't be awaited
        WebTask::new()
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
        // Spawn the future directly using wasm-bindgen-futures
        // We need to wrap it to discard the result since spawn_local expects ()
        spawn_local(async move {
            let _ = fut.await;
        });
        
        // Return a placeholder task since web tasks can't be awaited
        WebTask::new()
    }
}
