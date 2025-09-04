use crate::{Executor, LocalExecutor};
use core::future::Future;

impl Executor for async_executor::Executor<'static> {
    fn spawn<T: Send + 'static>(
        &self,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> async_task::Task<T> {
        async_executor::Executor::spawn(self, fut)
    }
}

impl LocalExecutor for async_executor::LocalExecutor<'static> {
    fn spawn<T: 'static>(&self, fut: impl Future<Output = T> + 'static) -> async_task::Task<T> {
        async_executor::LocalExecutor::spawn(self, fut)
    }
}
