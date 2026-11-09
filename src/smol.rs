//! A simple executor interface for async tasks.

use crate::Executor;

/// A global executor that uses the `smol` crate for task execution.
#[derive(Debug, Clone, Copy)]
pub struct SmolGlobal;

impl Executor for SmolGlobal {
    type Task<T: Send + 'static> = crate::async_task::AsyncTask<T>;

    fn spawn<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future<Output: Send> + Send + 'static,
    {
        smol::spawn(fut).into()
    }
}
