use std::{
    any::Any,
    borrow::Cow,
    mem::ManuallyDrop,
    ops::DerefMut,
    panic::{AssertUnwindSafe, catch_unwind},
    pin::Pin,
    task::{Poll, ready},
};

use alloc::{boxed::Box, string::String};

use pin_project_lite::pin_project;

#[allow(dead_code)]
pub type DefaultExecutor = ::async_executor::Executor<'static>;

#[allow(dead_code)]
pub type DefaultLocalExecutor = ::async_executor::LocalExecutor<'static>;

use crate::{Error, Executor, LocalExecutor, LocalTask, Task};

struct SmolTask<T>(ManuallyDrop<async_executor::Task<Result<T, Error>>>);

impl<T> Future for SmolTask<T> {
    type Output = T;
    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let result = ready!(Pin::new(self.0.deref_mut()).poll(cx));
        Poll::Ready(result.unwrap())
    }
}

impl<T: 'static> LocalTask for SmolTask<T> {
    async fn result(self) -> Result<Self::Output, crate::Error> {
        async_executor::Task::fallible(ManuallyDrop::into_inner(self.0))
            .await
            .unwrap_or(Err(Error::Cancelled))
    }

    fn cancel(self) {
        drop(ManuallyDrop::into_inner(self.0));
    }
}

impl<T: 'static + Send> Task for SmolTask<T> {
    async fn result(self) -> Result<Self::Output, crate::Error> {
        async_executor::Task::fallible(ManuallyDrop::into_inner(self.0))
            .await
            .unwrap_or(Err(Error::Cancelled))
    }

    fn cancel(self) {
        drop(ManuallyDrop::into_inner(self.0));
    }
}

impl Executor for async_executor::Executor<'static> {
    fn spawn<T: Send + 'static>(
        &self,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> impl Task<Output = T> {
        SmolTask(ManuallyDrop::new(async_executor::Executor::spawn(
            self,
            UnwindFuture { fut },
        )))
    }
}

pin_project! {
    struct UnwindFuture<Fut>{
        #[pin]
        fut:Fut
    }
}

impl<Fut: Future> Future for UnwindFuture<Fut> {
    type Output = Result<Fut::Output, Error>;
    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(
            match catch_unwind(AssertUnwindSafe(move || self.project().fut.poll(cx))) {
                Ok(value) => Ok(ready!(value)),
                Err(error) => Err(Error::Panicked(extract_error(error))),
            },
        )
    }
}

fn extract_error(error: Box<dyn Any>) -> Cow<'static, str> {
    error
        .downcast::<String>()
        .map(|s| Cow::Owned(*s))
        .unwrap_or_else(|error| {
            error
                .downcast::<&'static str>()
                .map(|s| Cow::Borrowed(*s))
                .unwrap_or(Cow::Borrowed("Task panicked"))
        })
}

impl LocalExecutor for async_executor::LocalExecutor<'static> {
    fn spawn<T: 'static>(
        &self,
        fut: impl Future<Output = T> + 'static,
    ) -> impl LocalTask<Output = T> {
        SmolTask(ManuallyDrop::new(async_executor::LocalExecutor::spawn(
            self,
            UnwindFuture { fut },
        )))
    }
}
