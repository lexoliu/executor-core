#![no_std]

mod async_executor;

#[cfg(feature = "tokio")]
mod tokio;

#[cfg(feature = "web")]
mod web;

use core::{
    fmt::Debug,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use alloc::boxed::Box;
use async_channel::Receiver;

extern crate alloc;

pub trait Executor: Send + Sync {
    type Task<T: Send + 'static>: Task<T> + Send;

    fn spawn<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future<Output: Send> + Send + 'static;
}

pub trait LocalExecutor {
    type Task<T: 'static>: Task<T>;

    fn spawn<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future + 'static;
}

trait AnyLocalExecutorImpl {
    fn spawn_boxed(
        &self,
        fut: Pin<Box<dyn Future<Output = ()>>>,
    ) -> Pin<Box<dyn Task<()> + 'static>>;
}

impl<E> AnyLocalExecutorImpl for E
where
    E: LocalExecutor + 'static,
{
    fn spawn_boxed(
        &self,
        fut: Pin<Box<dyn Future<Output = ()>>>,
    ) -> Pin<Box<dyn Task<()> + 'static>> {
        let task = self.spawn(fut);
        Box::pin(task)
    }
}

pub struct AnyLocalExecutor(Box<dyn AnyLocalExecutorImpl>);

impl Debug for AnyLocalExecutor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AnyLocalExecutor").finish()
    }
}

impl Debug for AnyExecutor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AnyExecutor").finish()
    }
}

impl AnyExecutor {
    pub fn new(executor: impl Executor + 'static) -> Self {
        Self(Box::new(executor))
    }
}

impl AnyLocalExecutor {
    pub fn new(executor: impl LocalExecutor + 'static) -> Self {
        Self(Box::new(executor))
    }
}

pub struct AnyLocalExecutorTask<T> {
    inner: Pin<Box<dyn Task<()> + 'static>>,
    receiver: Receiver<Result<T, Error>>,
}

impl<T> Future for AnyLocalExecutorTask<T> {
    type Output = T;

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        self.poll_result(cx).map(|res| res.unwrap())
    }
}

impl<T> Task<T> for AnyLocalExecutorTask<T> {
    fn poll_result(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<T, Error>> {
        let mut recv = self.receiver.recv();
        unsafe {
            Pin::new_unchecked(&mut recv)
                .poll(cx)
                .map(|res| res.unwrap())
        }
    }
    fn poll_cancel(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let this = unsafe { self.get_unchecked_mut() };
        this.inner.as_mut().poll_cancel(cx)
    }
}

impl LocalExecutor for AnyLocalExecutor {
    type Task<T: 'static> = AnyLocalExecutorTask<T>;

    fn spawn<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future + 'static,
    {
        let (sender, receiver) = async_channel::bounded(1);
        let fut = async move {
            let res = fut.await;
            let _ = sender.send(Ok(res)).await;
        };
        let inner = self.0.spawn_boxed(Box::pin(fut));
        AnyLocalExecutorTask { inner, receiver }
    }
}

type Error = Box<dyn core::any::Any + Send>;

pub trait Task<T>: Future<Output = T> {
    fn poll_result(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<T, Error>>;
    fn poll_cancel(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()>;

    fn result(self) -> impl Future<Output = Result<T, Error>>
    where
        Self: Sized,
    {
        ResultFuture {
            task: self,
            _phantom: PhantomData,
        }
    }

    fn cancel(self) -> impl Future<Output = ()>
    where
        Self: Sized,
    {
        CancelFuture {
            task: self,
            _phantom: PhantomData,
        }
    }
}

pub struct ResultFuture<T: Task<U>, U> {
    task: T,
    _phantom: PhantomData<U>,
}

impl<T: Task<U>, U> Future for ResultFuture<T, U> {
    type Output = Result<U, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        unsafe { Pin::new_unchecked(&mut this.task) }.poll_result(cx)
    }
}

pub struct CancelFuture<T: Task<U>, U> {
    task: T,
    _phantom: PhantomData<U>,
}

impl<T: Task<U>, U> Future for CancelFuture<T, U> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        unsafe { Pin::new_unchecked(&mut this.task) }.poll_cancel(cx)
    }
}

pub struct AnyExecutor(Box<dyn AnyExecutorImpl>);

pub struct AnyExecutorTask<T> {
    inner: Pin<Box<dyn Task<()> + Send>>,
    receiver: Receiver<Result<T, Error>>,
}

impl<T: Send> Future for AnyExecutorTask<T> {
    type Output = T;

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        self.poll_result(cx).map(|res| res.unwrap())
    }
}

impl<T: Send> Task<T> for AnyExecutorTask<T> {
    fn poll_result(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<T, Error>> {
        let mut recv = self.receiver.recv();
        unsafe {
            Pin::new_unchecked(&mut recv)
                .poll(cx)
                .map(|res| res.unwrap())
        }
    }
    fn poll_cancel(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let this = unsafe { self.get_unchecked_mut() };
        this.inner.as_mut().poll_cancel(cx)
    }
}

impl Executor for AnyExecutor {
    type Task<T: Send + 'static> = AnyExecutorTask<T>;

    fn spawn<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future<Output: Send> + Send + 'static,
    {
        let (sender, receiver) = async_channel::bounded(1);
        let fut = async move {
            let res = fut.await;
            let _ = sender.send(Ok(res)).await;
        };
        let inner = self.0.spawn_boxed(Box::pin(fut));
        AnyExecutorTask { inner, receiver }
    }
}

trait AnyExecutorImpl: Send + Sync + 'static {
    fn spawn_boxed(
        &self,
        fut: Pin<Box<dyn Future<Output = ()> + Send>>,
    ) -> Pin<Box<dyn Task<()> + Send>>;
}

impl<T: Task<T>> Task<T> for Pin<Box<T>> {
    fn poll_result(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<T, Error>> {
        let this = unsafe { self.get_unchecked_mut() };
        this.as_mut().poll_result(cx)
    }
    fn poll_cancel(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let this = unsafe { self.get_unchecked_mut() };

        this.as_mut().poll_cancel(cx)
    }
}

impl<E> AnyExecutorImpl for E
where
    E: Executor + 'static,
{
    fn spawn_boxed(
        &self,
        fut: Pin<Box<dyn Future<Output = ()> + Send>>,
    ) -> Pin<Box<dyn Task<()> + Send>> {
        let task = self.spawn(fut);
        Box::pin(task)
    }
}

mod std_on {
    use alloc::boxed::Box;

    use crate::{
        AnyExecutor, AnyExecutorTask, AnyLocalExecutor, AnyLocalExecutorTask, Executor,
        LocalExecutor,
    };

    extern crate std;

    use core::{cell::OnceCell, panic::AssertUnwindSafe};
    use std::sync::OnceLock;
    std::thread_local! {
        static LOCAL_EXECUTOR: OnceCell<AnyLocalExecutor> = const { OnceCell::new() };
    }
    pub fn init_local_executor(executor: impl LocalExecutor + 'static) {
        LOCAL_EXECUTOR.with(|cell| {
            cell.set(AnyLocalExecutor::new(executor))
                .expect("Local executor already set");
        });
    }

    static GLOBAL_EXECUTOR: OnceLock<AnyExecutor> = OnceLock::new();

    pub fn init_global_executor(executor: impl crate::Executor + 'static) {
        GLOBAL_EXECUTOR
            .set(AnyExecutor::new(executor))
            .expect("Global executor already set");
    }

    pub fn spawn<Fut>(fut: Fut) -> AnyExecutorTask<Fut::Output>
    where
        Fut: Future<Output: Send> + Send + 'static,
    {
        let executor = GLOBAL_EXECUTOR.get().expect("Global executor not set");
        executor.spawn(fut)
    }

    pub fn spawn_local<Fut>(fut: Fut) -> AnyLocalExecutorTask<Fut::Output>
    where
        Fut: Future + 'static,
    {
        LOCAL_EXECUTOR.with(|cell| {
            let executor = cell.get().expect("Local executor not set");
            executor.spawn(fut)
        })
    }

    pub(crate) fn catch_unwind<F, R>(f: F) -> Result<R, Box<dyn std::any::Any + Send>>
    where
        F: FnOnce() -> R,
    {
        std::panic::catch_unwind(AssertUnwindSafe(f))
    }
}

pub use std_on::*;

// Re-export executors and tasks
pub use async_executor::AsyncTask;

#[cfg(feature = "tokio")]
pub use tokio::{DefaultExecutor, TokioLocalTask, TokioTask};

#[cfg(feature = "web")]
pub use web::{WebExecutor, WebTask};
