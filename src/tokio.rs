use std::{
    pin::Pin,
    task::{Poll, ready},
};

use crate::{Error, Executor, LocalTask, Task};

impl Executor for tokio::runtime::Runtime {
    fn spawn<T: Send + 'static>(
        &self,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> impl Task<Output = T> {
        TokioTask(tokio::runtime::Runtime::spawn(self, fut))
    }
}

struct TokioTask<T>(tokio::task::JoinHandle<T>);

impl<T: 'static> LocalTask for TokioTask<T> {
    async fn result(self) -> Result<Self::Output, crate::Error> {
        self.0.await.map_err(|error| {
            if let Err(panic) = error.try_into_panic() {
                Error::Panicked(panic.to_string().into())
            } else {
                Error::Cancelled
            }
        })
    }
    async fn cancel(self) -> Option<Self::Output> {
        self.0.abort();
        None
    }
}

impl<T: 'static + Send> Task for TokioTask<T> {
    async fn result(self) -> Result<Self::Output, crate::Error> {
        self.0.await.map_err(|error| {
            if let Err(panic) = error.try_into_panic() {
                Error::Panicked(panic.to_string().into())
            } else {
                Error::Cancelled
            }
        })
    }
    async fn cancel(self) -> Option<Self::Output> {
        self.0.abort();
        None
    }
}

impl<T> Future for TokioTask<T> {
    type Output = T;
    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let result = ready!(Pin::new(&mut self.0).poll(cx));

        Poll::Ready(result.unwrap())
    }
}
