//! # executor-core
//!
//! A flexible task executor abstraction layer for Rust async runtimes.
//!
//! This crate provides unified traits and type-erased wrappers for different async executors,
//! allowing you to write code that's agnostic to the underlying executor implementation.
//!
//! ## Overview
//!
//! The crate is built around two main traits:
//! - [`Executor`]: For spawning `Send + 'static` futures
//! - [`LocalExecutor`]: For spawning `'static` futures (not necessarily `Send`)
//!
//! Both traits produce tasks that implement the [`Task`] trait, providing:
//! - [`Future`] implementation for awaiting results
//! - [`Task::poll_result`] for explicit error handling
//! - [`Task::poll_cancel`] for task cancellation
//!
//! ## Quick Start
//!
//! ```rust
//! use executor_core::{Executor, init_global_executor, spawn};
//! use executor_core::tokio::TokioExecutor;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Initialize the global executor
//!     init_global_executor(TokioExecutor::new());
//!
//!     // Spawn a task
//!     let task = spawn(async {
//!         println!("Hello from spawned task!");
//!         42
//!     });
//!
//!     // The task can be awaited to get the result
//!     let result = task.await;
//!     println!("Task result: {}", result);
//! }
//! ```
//!
//! ## Features
//!
//! - **Zero-cost Executor Abstraction**: Unified [`Executor`] and [`LocalExecutor`] traits
//!   using Generic Associated Types (GAT) to prevent unnecessary heap allocation and dynamic dispatch
//! - **Type Erasure**: [`AnyExecutor`] and [`AnyLocalExecutor`] for runtime flexibility
//! - **Task Management**: Rich task API with cancellation and error handling
//! - **No-std Compatible**: Core functionality works in no-std environments
//! - **Panic Safety**: Proper panic handling and propagation
//!
//! ## Lifetime Constraints
//!
//! The current API requires `'static` lifetimes for both futures and their outputs.
//! This constraint comes from the underlying async runtimes and ensures memory safety
//! when tasks may outlive their spawning scope. While this limits flexibility, it
//! matches the constraints of most async runtime implementations in Rust.

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs, missing_debug_implementations)]

#[cfg(feature = "async-task")]
#[cfg_attr(docsrs, doc(cfg(feature = "async-task")))]
pub mod async_task;

#[cfg(feature = "async-executor")]
#[cfg_attr(docsrs, doc(cfg(feature = "async-executor")))]
pub mod async_executor;

#[cfg(feature = "tokio")]
#[cfg_attr(docsrs, doc(cfg(feature = "tokio")))]
pub mod tokio;

use core::{
    any::Any,
    fmt::Debug,
    future::{Future, poll_fn},
    marker::PhantomData,
    panic::AssertUnwindSafe,
    pin::Pin,
    task::{Context, Poll},
};

pub mod mailbox;

use alloc::boxed::Box;
use async_channel::Receiver;

extern crate alloc;

/// A trait for spawning `Send + 'static` futures.
///
/// This trait is implemented by runtime-agnostic executors that can spawn futures
/// across thread boundaries. The spawned futures must be `Send` and `'static`.
///
/// The `'static` lifetime requirements come from the underlying async runtimes
/// (like Tokio) which need to ensure memory safety when tasks are moved across
/// threads and may outlive their spawning scope.
///
/// See [AnyExecutor] for a type-erased executor.
pub trait Executor: Send + Sync {
    /// The task type returned by [`spawn`](Self::spawn).
    ///
    /// The `T: Send + 'static` constraint ensures the task output can be safely
    /// sent across thread boundaries and doesn't contain any borrowed data.
    type Task<T: Send + 'static>: Task<T> + Send;

    /// Spawn a future that will run to completion.
    ///
    /// The future must be `Send + 'static` to ensure it can be moved across threads.
    /// Returns a [`Task`] that can be awaited to get the result.
    fn spawn<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future<Output: Send> + Send + 'static;
}

impl<E: Executor> Executor for &E {
    type Task<T: Send + 'static> = E::Task<T>;

    fn spawn<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future<Output: Send> + Send + 'static,
    {
        (*self).spawn(fut)
    }
}

impl<E: Executor> Executor for &mut E {
    type Task<T: Send + 'static> = E::Task<T>;

    fn spawn<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future<Output: Send> + Send + 'static,
    {
        (**self).spawn(fut)
    }
}

impl<E: Executor> Executor for Box<E> {
    type Task<T: Send + 'static> = E::Task<T>;

    fn spawn<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future<Output: Send> + Send + 'static,
    {
        (**self).spawn(fut)
    }
}

impl<E: Executor> Executor for alloc::sync::Arc<E> {
    type Task<T: Send + 'static> = E::Task<T>;

    fn spawn<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future<Output: Send> + Send + 'static,
    {
        (**self).spawn(fut)
    }
}

/// A trait for spawning `'static` futures that may not be `Send`.
///
/// This trait is for executors that can spawn futures that don't need to be `Send`,
/// typically single-threaded executors or local task spawners.
///
/// The `'static` lifetime requirements come from the underlying async runtimes
/// which need to ensure memory safety when tasks may outlive their spawning scope,
/// even in single-threaded contexts.
///
/// See [AnyLocalExecutor] for a type-erased local executor.
pub trait LocalExecutor {
    /// The task type returned by [`spawn`](Self::spawn).
    ///
    /// The `T: 'static` constraint ensures the task output doesn't contain
    /// any borrowed data that could become invalid.
    type Task<T: 'static>: Task<T>;

    /// Spawn a future that will run to completion on the local executor.
    ///
    /// The future must be `'static` but does not need to be `Send`.
    /// Returns a [`Task`] that can be awaited to get the result.
    fn spawn_local<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future + 'static;
}

impl<E: LocalExecutor> LocalExecutor for &E {
    type Task<T: 'static> = E::Task<T>;

    fn spawn_local<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future + 'static,
    {
        (*self).spawn_local(fut)
    }
}

impl<E: LocalExecutor> LocalExecutor for &mut E {
    type Task<T: 'static> = E::Task<T>;

    fn spawn_local<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future + 'static,
    {
        (**self).spawn_local(fut)
    }
}

impl<E: LocalExecutor> LocalExecutor for Box<E> {
    type Task<T: 'static> = E::Task<T>;

    fn spawn_local<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future + 'static,
    {
        (**self).spawn_local(fut)
    }
}

impl<E: LocalExecutor> LocalExecutor for alloc::rc::Rc<E> {
    type Task<T: 'static> = E::Task<T>;

    fn spawn_local<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future + 'static,
    {
        (**self).spawn_local(fut)
    }
}

impl<E: LocalExecutor> LocalExecutor for alloc::sync::Arc<E> {
    type Task<T: 'static> = E::Task<T>;

    fn spawn_local<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future + 'static,
    {
        (**self).spawn_local(fut)
    }
}

trait AnyLocalExecutorImpl: 'static + Any {
    fn spawn_local_boxed(
        &self,
        fut: Pin<Box<dyn Future<Output = ()>>>,
    ) -> Pin<Box<dyn Task<()> + 'static>>;
}

impl<E> AnyLocalExecutorImpl for E
where
    E: LocalExecutor + 'static,
{
    fn spawn_local_boxed(
        &self,
        fut: Pin<Box<dyn Future<Output = ()>>>,
    ) -> Pin<Box<dyn Task<()> + 'static>> {
        let task = self.spawn_local(fut);
        Box::pin(task)
    }
}

/// A type-erased [`LocalExecutor`] that can hold any local executor implementation.
///
/// This allows for runtime selection of executors and storing different executor
/// types in the same collection.
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

impl<T> dyn Task<T> {
    /// Get the result of the boxed task, including any errors that occurred.
    ///
    /// This method awaits the task completion and returns a [`Result`] that
    /// allows you to handle panics and other errors explicitly.
    pub async fn result(self: Box<Self>) -> Result<T, Error> {
        let mut pinned: Pin<Box<Self>> = self.into();

        poll_fn(move |cx| pinned.as_mut().poll_result(cx)).await
    }
}

impl AnyExecutor {
    /// Create a new [`AnyExecutor`] wrapping the given executor.
    pub fn new(executor: impl Executor + 'static) -> Self {
        Self(Box::new(executor))
    }

    /// Attempt to downcast to a concrete executor type by reference.
    ///
    /// Returns `Some(&E)` if the underlying executor is of type `E`, `None` otherwise.
    pub fn downcast_ref<E: Executor + 'static>(&self) -> Option<&E> {
        let any: &dyn Any = self.0.as_ref();

        any.downcast_ref()
    }

    /// Attempt to downcast to a concrete executor type by value.
    ///
    /// Returns `Ok(Box<E>)` if the underlying executor is of type `E`,
    /// `Err(Self)` otherwise (returning the original `AnyExecutor`).
    pub fn downcast<E: Executor + 'static>(self) -> Result<Box<E>, Self> {
        if (&self.0 as &dyn Any).is::<E>() {
            Ok((self.0 as Box<dyn Any>).downcast().ok().unwrap())
        } else {
            Err(self)
        }
    }
}

impl AnyLocalExecutor {
    /// Create a new [`AnyLocalExecutor`] wrapping the given local executor.
    pub fn new(executor: impl LocalExecutor + 'static) -> Self {
        Self(Box::new(executor))
    }

    /// Attempt to downcast to a concrete local executor type by reference.
    ///
    /// Returns `Some(&E)` if the underlying executor is of type `E`, `None` otherwise.
    pub fn downcast_ref<E: LocalExecutor + 'static>(&self) -> Option<&E> {
        let any: &dyn Any = self.0.as_ref();

        any.downcast_ref()
    }

    /// Attempt to downcast to a concrete local executor type by value.
    ///
    /// Returns `Ok(Box<E>)` if the underlying executor is of type `E`,
    /// `Err(Self)` otherwise (returning the original `AnyLocalExecutor`).
    pub fn downcast<E: LocalExecutor + 'static>(self) -> Result<Box<E>, Self> {
        if (&self.0 as &dyn Any).is::<E>() {
            Ok((self.0 as Box<dyn Any>).downcast().ok().unwrap())
        } else {
            Err(self)
        }
    }
}

/// Task type returned by [`AnyLocalExecutor`].
///
/// This task can be awaited like any other task and provides the same
/// cancellation and error handling capabilities as other task implementations.
/// It wraps tasks from any [`LocalExecutor`] implementation in a type-erased manner.
pub struct AnyLocalExecutorTask<T> {
    inner: Pin<Box<dyn Task<()> + 'static>>,
    receiver: Receiver<Result<T, Error>>,
}

impl<T> AnyLocalExecutorTask<T> {
    /// Create a new `AnyLocalExecutorTask` wrapping the given inner task and receiver.
    fn new(inner: Pin<Box<dyn Task<()> + 'static>>, receiver: Receiver<Result<T, Error>>) -> Self {
        Self { inner, receiver }
    }

    /// Get the result of the task, including any errors that occurred.
    pub async fn result(self) -> Result<T, Error> {
        <Self as Task<T>>::result(self).await
    }

    /// Detach the task, allowing it to run in the background without being awaited.
    pub fn detach(self) {
        <Self as Task<T>>::detach(self)
    }
}

impl<T> core::fmt::Debug for AnyLocalExecutorTask<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AnyLocalExecutorTask")
            .finish_non_exhaustive()
    }
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
        // First, ensure the underlying task is being polled
        let this = unsafe { self.get_unchecked_mut() };
        let _ = this.inner.as_mut().poll(cx);

        // Then poll the receiver
        let mut recv = this.receiver.recv();
        unsafe {
            Pin::new_unchecked(&mut recv)
                .poll(cx)
                .map(|res| res.unwrap_or_else(|_| Err(Box::new("Channel closed"))))
        }
    }
}

impl LocalExecutor for AnyLocalExecutor {
    type Task<T: 'static> = AnyLocalExecutorTask<T>;

    fn spawn_local<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
    where
        Fut: Future + 'static,
    {
        let (sender, receiver) = async_channel::bounded(1);
        let fut = async move {
            let res = AssertUnwindSafe(fut).await;
            let _ = sender.send(Ok(res)).await;
        };
        let inner = self.0.spawn_local_boxed(Box::pin(fut));
        AnyLocalExecutorTask::new(inner, receiver)
    }
}

/// Type alias for errors that can occur during task execution.
///
/// This represents panics or other unrecoverable errors from spawned tasks.
type Error = Box<dyn core::any::Any + Send>;

/// A trait representing a spawned task that can be awaited, cancelled, or queried for results.
///
/// This trait extends [`Future`] with additional capabilities for task management:
/// - Explicit error handling via [`poll_result`](Self::poll_result)
/// - Task cancellation via [`poll_cancel`](Self::poll_cancel)
/// - Convenience methods for getting results and cancelling
///
/// `Task` would be cancelled when dropped.
pub trait Task<T>: Future<Output = T> {
    /// Poll the task for completion, returning a [`Result`] that can contain errors.
    ///
    /// Unlike the [`Future::poll`] implementation, this method allows you to handle
    /// task panics and other errors explicitly rather than propagating them.
    fn poll_result(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<T, Error>>;

    /// Get the result of the task, including any errors that occurred.
    ///
    /// This is equivalent to awaiting the task but returns a [`Result`] that
    /// allows you to handle panics and other errors explicitly.
    ///
    fn result(self) -> impl Future<Output = Result<T, Error>>
    where
        Self: Sized,
    {
        ResultFuture {
            task: self,
            _phantom: PhantomData,
        }
    }

    /// Detach the task, allowing it to run in the background without being awaited.
    ///
    /// Once detached, the task will continue running but its result cannot be retrieved.
    /// This is useful for fire-and-forget operations where you don't need to wait for
    /// or handle the result.
    fn detach(self)
    where
        Self: Sized,
    {
        core::mem::forget(self);
    }
}

/// Future returned by [`Task::result()`].
///
/// This future resolves to a `Result<T, Error>` when the underlying task completes,
/// allowing explicit handling of task panics and other errors without propagating them.
pub struct ResultFuture<T: Task<U>, U> {
    task: T,
    _phantom: PhantomData<U>,
}

impl<T: Task<U>, U> core::fmt::Debug for ResultFuture<T, U> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ResultFuture").finish_non_exhaustive()
    }
}

impl<T: Task<U>, U> Future for ResultFuture<T, U> {
    type Output = Result<U, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        unsafe { Pin::new_unchecked(&mut this.task) }.poll_result(cx)
    }
}

/// A type-erased [`Executor`] that can hold any executor implementation.
///
/// This allows for runtime selection of executors and storing different executor
/// types in the same collection.
///
pub struct AnyExecutor(Box<dyn AnyExecutorImpl>);

/// Task type returned by [`AnyExecutor`].
///
/// This task can be awaited like any other task and provides the same
/// cancellation and error handling capabilities.
pub struct AnyExecutorTask<T> {
    inner: Pin<Box<dyn Task<()> + Send>>,
    receiver: Receiver<Result<T, Error>>,
}

impl<T: Send> AnyExecutorTask<T> {
    /// Create a new `AnyExecutorTask` wrapping the given inner task and receiver.
    fn new(inner: Pin<Box<dyn Task<()> + Send>>, receiver: Receiver<Result<T, Error>>) -> Self {
        Self { inner, receiver }
    }

    /// Get the result of the task, including any errors that occurred.
    ///
    /// This is equivalent to awaiting the task but returns a [`Result`] that
    /// allows you to handle panics and other errors explicitly.
    pub async fn result(self) -> Result<T, Error> {
        <Self as Task<T>>::result(self).await
    }

    /// Detach the task, allowing it to run in the background without being awaited.
    pub fn detach(self) {
        <Self as Task<T>>::detach(self)
    }
}

impl<T> core::fmt::Debug for AnyExecutorTask<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AnyExecutorTask").finish_non_exhaustive()
    }
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
        // First, ensure the underlying task is being polled
        let this = unsafe { self.get_unchecked_mut() };
        let _ = this.inner.as_mut().poll(cx);

        // Then poll the receiver
        let mut recv = this.receiver.recv();
        unsafe {
            Pin::new_unchecked(&mut recv)
                .poll(cx)
                .map(|res| res.unwrap_or_else(|_| Err(Box::new("Channel closed"))))
        }
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
            let res = AssertUnwindSafe(fut).await;
            let _ = sender.send(Ok(res)).await;
        };
        let inner = self.0.spawn_boxed(Box::pin(fut));
        AnyExecutorTask::new(inner, receiver)
    }
}

trait AnyExecutorImpl: Send + Sync + Any {
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

#[cfg(feature = "std")]
mod std_on {
    use alloc::boxed::Box;

    use crate::{
        AnyExecutor, AnyExecutorTask, AnyLocalExecutor, AnyLocalExecutorTask, Executor,
        LocalExecutor,
    };

    extern crate std;

    use core::{cell::OnceCell, future::Future, panic::AssertUnwindSafe};
    use std::sync::OnceLock;
    std::thread_local! {
        static LOCAL_EXECUTOR: OnceCell<AnyLocalExecutor> = const { OnceCell::new() };
    }

    /// Initialize the thread-local executor for spawning non-Send futures.
    ///
    /// This must be called before using [`spawn_local`]. The executor will be used
    /// for all [`spawn_local`] calls on the current thread.
    ///
    /// # Panics
    ///
    /// Panics if a local executor has already been set for this thread.
    ///
    pub fn init_local_executor(executor: impl LocalExecutor + 'static) {
        if try_init_local_executor(executor).is_err() {
            panic!("Local executor already set for this thread");
        }
    }

    /// Try to initialize the thread-local executor for spawning non-Send futures.
    ///
    /// This is a non-panicking version of [`init_local_executor`].
    pub fn try_init_local_executor<E>(executor: E) -> Result<(), E>
    where
        E: LocalExecutor + 'static,
    {
        LOCAL_EXECUTOR.with(|cell| {
            cell.set(AnyLocalExecutor::new(executor))
                .map_err(|e| *e.downcast().unwrap())
        })
    }

    static GLOBAL_EXECUTOR: OnceLock<AnyExecutor> = OnceLock::new();

    /// Initialize the global executor for spawning Send futures.
    ///
    /// This must be called before using [`spawn`]. The executor will be used
    /// for all [`spawn`] calls across all threads.
    ///
    /// # Panics
    ///
    /// Panics if a global executor has already been set.
    ///
    pub fn init_global_executor(executor: impl crate::Executor + 'static) {
        if GLOBAL_EXECUTOR.set(AnyExecutor::new(executor)).is_err() {
            panic!("Global executor already set");
        }
    }

    /// Try to initialize the global executor for spawning Send futures.
    ///
    /// This is a non-panicking version of [`init_global_executor`].
    pub fn try_init_global_executor<E>(executor: E) -> Result<(), E>
    where
        E: crate::Executor + 'static,
    {
        GLOBAL_EXECUTOR
            .set(AnyExecutor::new(executor))
            .map_err(|e| *e.downcast().unwrap())
    }

    /// Spawn a `Send` future on the global executor.
    ///
    /// The global executor must be initialized with [`init_global_executor`] before
    /// calling this function.
    ///
    /// # Panics
    ///
    /// Panics if the global executor has not been set.
    ///
    pub fn spawn<Fut>(fut: Fut) -> AnyExecutorTask<Fut::Output>
    where
        Fut: Future<Output: Send> + Send + 'static,
    {
        let executor = GLOBAL_EXECUTOR.get().expect("Global executor not set");
        executor.spawn(fut)
    }

    /// Spawn a future on the thread-local executor.
    ///
    /// The local executor must be initialized with [`init_local_executor`] before
    /// calling this function. Unlike [`spawn`], this can handle futures that are
    /// not `Send`.
    ///
    /// # Panics
    ///
    /// Panics if the local executor has not been set for this thread.
    ///
    pub fn spawn_local<Fut>(fut: Fut) -> AnyLocalExecutorTask<Fut::Output>
    where
        Fut: Future + 'static,
    {
        LOCAL_EXECUTOR.with(|cell| {
            let executor = cell.get().expect("Local executor not set");
            executor.spawn_local(fut)
        })
    }

    #[allow(unused)]
    pub(crate) fn catch_unwind<F, R>(f: F) -> Result<R, Box<dyn std::any::Any + Send>>
    where
        F: FnOnce() -> R,
    {
        std::panic::catch_unwind(AssertUnwindSafe(f))
    }

    /// A default executor that uses the global and local executors.
    ///
    /// This executor delegates to the global executor for `Send` futures
    /// and the local executor for non-`Send` futures.
    #[derive(Clone, Copy, Debug)]
    pub struct DefaultExecutor;

    impl Executor for DefaultExecutor {
        type Task<T: Send + 'static> = AnyExecutorTask<T>;

        fn spawn<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
        where
            Fut: core::future::Future<Output: Send> + Send + 'static,
        {
            spawn(fut)
        }
    }

    impl LocalExecutor for DefaultExecutor {
        type Task<T: 'static> = AnyLocalExecutorTask<T>;

        fn spawn_local<Fut>(&self, fut: Fut) -> Self::Task<Fut::Output>
        where
            Fut: core::future::Future + 'static,
        {
            spawn_local(fut)
        }
    }
}

#[cfg(feature = "std")]
pub use std_on::*;
