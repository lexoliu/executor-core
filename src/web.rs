use crate::{Executor, LocalExecutor};
use core::future::Future;
use wasm_bindgen_futures::spawn_local;

pub struct Web;

impl LocalExecutor for Web {
    fn spawn<T: 'static>(
        &self,
        fut: impl Future<Output = T> + 'static,
    ) -> async_task::Task<T> {
        let (runnable, task) = async_task::spawn_local(fut, |runnable: async_task::Runnable| {
            spawn_local(async move {
                runnable.run();
            });
        });
        runnable.schedule();
        task
    }
}

impl Executor for Web {
    fn spawn<T: Send + 'static>(
        &self,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> async_task::Task<T> {
        // In web environment, we use spawn_local even for Send futures
        // since web workers don't have the same threading model as native
        let (runnable, task) = async_task::spawn_local(fut, |runnable: async_task::Runnable| {
            spawn_local(async move {
                runnable.run();
            });
        });
        runnable.schedule();
        task
    }
}