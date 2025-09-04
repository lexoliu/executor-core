use crate::Executor;
use core::future::Future;
use std::sync::Arc;

impl Executor for tokio::runtime::Runtime {
    fn spawn<T: Send + 'static>(
        &self,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> async_task::Task<T> {
        spawn_with_handle(tokio::runtime::Runtime::spawn(self, fut))
    }
}

impl Executor for Arc<tokio::runtime::Runtime> {
    fn spawn<T: Send + 'static>(
        &self,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> async_task::Task<T> {
        spawn_with_handle(self.as_ref().spawn(fut))
    }
}

impl Executor for &tokio::runtime::Runtime {
    fn spawn<T: Send + 'static>(
        &self,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> async_task::Task<T> {
        spawn_with_handle(tokio::runtime::Runtime::spawn(self, fut))
    }
}

fn spawn_with_handle<T: Send + 'static>(handle: tokio::task::JoinHandle<T>) -> async_task::Task<T> {
    let (runnable, task) = async_task::spawn(
        async move { handle.await.expect("Tokio task panicked") },
        |runnable: async_task::Runnable| {
            tokio::spawn(async move {
                runnable.run();
            });
        },
    );
    runnable.schedule();
    task
}
