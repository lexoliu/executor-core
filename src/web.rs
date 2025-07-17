use core::{
    pin::Pin,
    task::{Context, Poll},
};

use crate::{Executor, LocalExecutor, LocalTask};
use async_oneshot::Receiver;
use wasm_bindgen_futures::spawn_local;
pub struct Web;

pub struct Task<T>(SingleThread<Receiver<T>>);

impl<T: Future> Future for SingleThread<T> {
    type Output = T::Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        // SAFETY: We ensure that the inner future is pinned and safe to poll.
        unsafe { Pin::new_unchecked(&mut this.0).poll(cx) }
    }
}

struct SingleThread<T>(T);

unsafe impl<T> Send for SingleThread<T> {}
unsafe impl<T> Sync for SingleThread<T> {}

unsafe impl<T: Send> Send for Task<T> {}

impl<T> Future for Task<T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe {
            Pin::new_unchecked(&mut self.0)
                .poll(cx)
                .map_err(|_| crate::Error::Cancelled)
                .map(|result| result.expect("Task should not be cancelled"))
        }
    }
}

impl<T: 'static> LocalTask for Task<T> {
    async fn result(self) -> Result<T, crate::Error> {
        self.0.await.map_err(|_| crate::Error::Cancelled)
    }

    fn cancel(self) {
        // In a web context, cancellation is not directly supported.
        // This is a no-op.
    }
}

impl<T: Send + 'static> crate::Task for Task<T> {
    async fn result(self) -> Result<T, crate::Error> {
        self.0.await.map_err(|_| crate::Error::Cancelled)
    }

    fn cancel(self) {
        // In a web context, cancellation is not directly supported.
        // This is a no-op.
    }
}

impl LocalExecutor for Web {
    fn spawn<T: 'static>(
        &self,
        fut: impl Future<Output = T> + 'static,
    ) -> impl crate::LocalTask<Output = T> {
        let (mut sender, receiver) = async_oneshot::oneshot();
        spawn_local(async move {
            let result = fut.await;
            sender
                .send(result)
                .expect("Failed to send result from task")
        });
        Task(SingleThread(receiver))
    }
}

impl Executor for Web {
    fn spawn<T: Send + 'static>(
        &self,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> impl crate::Task<Output = T> {
        let (mut sender, receiver) = async_oneshot::oneshot();
        spawn_local(async move {
            let result = fut.await;
            sender
                .send(result)
                .expect("Failed to send result from task")
        });
        Task(SingleThread(receiver))
    }
}
