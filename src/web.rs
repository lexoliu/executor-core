use crate::{Executor, LocalExecutor, Task, async_executor::AsyncTask};
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use wasm_bindgen_futures::spawn_local;

/// Web-based executor implementation for WASM targets
#[derive(Clone)]
pub struct WebExecutor;

impl WebExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WebExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Task wrapper for web/WASM environment using async_task
pub struct WebTask<T>(AsyncTask<T>);

impl<T> Future for WebTask<T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

impl<T> Task<T> for WebTask<T> {
    fn poll_result(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<T, crate::Error>> {
        Pin::new(&mut self.0).poll_result(cx)
    }

    fn poll_cancel(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        Pin::new(&mut self.0).poll_cancel(cx)
    }
}

impl LocalExecutor for WebExecutor {
    type Task<T: 'static> = WebTask<T>;

    fn spawn<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future + 'static,
    {
        let (runnable, task) = async_task::spawn_local(fut, |runnable: async_task::Runnable| {
            spawn_local(async move {
                runnable.run();
            });
        });
        runnable.schedule();
        WebTask(AsyncTask::from(task))
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
        WebTask(AsyncTask::from(task))
    }
}
