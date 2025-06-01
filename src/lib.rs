use std::sync::LazyLock;

#[cfg(feature = "async-executor")]
mod async_executor;
#[cfg(feature = "tokio")]
mod tokio;

#[cfg(feature = "default-async-executor")]
pub type DefaultExecutor = ::async_executor::Executor<'static>;

#[cfg(feature = "default-async-executor")]
pub type DefaultLocalExecutor = ::async_executor::LocalExecutor<'static>;

pub trait Executor {
    fn spawn<T: Send + 'static>(
        &self,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> impl Send + Task<Output = T>;
}

pub trait LocalExecutor {
    fn spawn<T: 'static>(&self, fut: impl Future<Output = T> + 'static) -> impl Task<Output = T>;
}

pub trait Task: Future + 'static {
    fn detach(self);
}

static GLOBAL_EXECUTOR: LazyLock<DefaultExecutor> = LazyLock::new(DefaultExecutor::default);

pub fn spawn<T: Send + 'static>(
    fut: impl Future<Output = T> + Send + 'static,
) -> impl Task<Output = T> {
    GLOBAL_EXECUTOR.spawn(fut)
}
