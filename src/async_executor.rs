use crate::{Error, Executor, LocalExecutor, LocalTask, Task};

impl<T: 'static> LocalTask for async_executor::Task<T> {
    async fn result(self) -> Result<Self::Output, crate::Error> {
        async_executor::Task::fallible(self)
            .await
            .ok_or(Error::Cancelled)
    }
    async fn cancel(self) -> Option<Self::Output> {
        async_executor::Task::cancel(self).await
    }
}

impl<T: 'static + Send> Task for async_executor::Task<T> {
    async fn result(self) -> Result<Self::Output, crate::Error> {
        async_executor::Task::fallible(self)
            .await
            .ok_or(Error::Cancelled)
    }
    async fn cancel(self) -> Option<Self::Output> {
        async_executor::Task::cancel(self).await
    }
}

impl Executor for async_executor::Executor<'static> {
    fn spawn<T: Send + 'static>(
        &self,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> impl Task<Output = T> {
        async_executor::Executor::spawn(self, fut)
    }
}

impl LocalExecutor for async_executor::LocalExecutor<'static> {
    fn spawn<T: 'static>(
        &self,
        fut: impl Future<Output = T> + 'static,
    ) -> impl LocalTask<Output = T> {
        async_executor::LocalExecutor::spawn(self, fut)
    }
}
