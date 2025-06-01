use crate::{Executor, LocalExecutor, Task};

impl<T: 'static> Task for async_executor::Task<T> {
    fn detach(self) {
        async_executor::Task::detach(self);
    }
}

impl Executor for async_executor::Executor<'static> {
    fn spawn<T: Send + 'static>(
        &self,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> impl Send + Task<Output = T> {
        async_executor::Executor::spawn(self, fut)
    }
}

impl LocalExecutor for async_executor::LocalExecutor<'static> {
    fn spawn<T: 'static>(&self, fut: impl Future<Output = T> + 'static) -> impl Task<Output = T> {
        async_executor::LocalExecutor::spawn(self, fut)
    }
}
