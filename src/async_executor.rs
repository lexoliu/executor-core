use crate::{Executor, LocalExecutor, Task, catch_unwind};
use core::{future::Future, mem::ManuallyDrop, pin::pin, task::Poll};

pub struct AsyncTask<T>(ManuallyDrop<Option<async_task::Task<T>>>);

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
