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
//! ```ignore
//! use executor_core::{Executor, init_global_executor, spawn};
//! use executor_core::tokio::DefaultExecutor;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Initialize the global executor
//!     init_global_executor(DefaultExecutor::new());
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
//! - **Multiple Runtime Support**: Tokio, async-executor, Web/WASM
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

#[cfg(feature = "async-executor")]
#[cfg_attr(docsrs, doc(cfg(feature = "async-executor")))]
pub mod async_executor;

#[cfg(feature = "tokio")]
#[cfg_attr(docsrs, doc(cfg(feature = "tokio")))]
pub mod tokio;

#[cfg(feature = "web")]
#[cfg_attr(docsrs, doc(cfg(feature = "web")))]
pub mod web;

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
#[cfg_attr(feature = "web", doc = "- [`web::WebExecutor`] for WASM environments")]
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
#[cfg_attr(feature = "web", doc = "- [`web::WebExecutor`] for WASM environments")]
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

/// A type-erased [`LocalExecutor`] that can hold any local executor implementation.
///
/// This allows for runtime selection of executors and storing different executor
/// types in the same collection.
///
/// # Examples
///
/// ```ignore
/// use executor_core::{AnyLocalExecutor, LocalExecutor};
/// use executor_core::tokio::DefaultExecutor;
///
/// let executor = DefaultExecutor::new();
/// let any_executor = AnyLocalExecutor::new(executor);
///
/// let task = any_executor.spawn(async { "Hello from any executor!" });
/// let result = task.await;
/// ```
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
    /// Create a new [`AnyExecutor`] wrapping the given executor.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use executor_core::{AnyExecutor, Executor};
    /// use executor_core::tokio::DefaultExecutor;
    ///
    /// let executor = DefaultExecutor::new();
    /// let any_executor = AnyExecutor::new(executor);
    ///
    /// let task = any_executor.spawn(async { 42 });
    /// let result = task.await;
    /// ```
    pub fn new(executor: impl Executor + 'static) -> Self {
        Self(Box::new(executor))
    }
}

impl AnyLocalExecutor {
    /// Create a new [`AnyLocalExecutor`] wrapping the given local executor.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use executor_core::{AnyLocalExecutor, LocalExecutor};
    /// use executor_core::tokio::DefaultExecutor;
    ///
    /// let executor = DefaultExecutor::new();
    /// let any_executor = AnyLocalExecutor::new(executor);
    ///
    /// let task = any_executor.spawn(async { 42 });
    /// let result = task.await;
    /// ```
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
/// # Examples
///
/// Basic usage:
/// ```ignore
/// use executor_core::{Executor, Task};
/// use executor_core::tokio::DefaultExecutor;
///
/// let executor = DefaultExecutor::new();
/// let task = executor.spawn(async { 42 });
///
/// // The task implements Future and can be awaited
/// let result = task.await;
/// ```
///
/// For error handling, see the [`result`](Self::result) method.
pub trait Task<T>: Future<Output = T> {
    /// Poll the task for completion, returning a [`Result`] that can contain errors.
    ///
    /// Unlike the [`Future::poll`] implementation, this method allows you to handle
    /// task panics and other errors explicitly rather than propagating them.
    fn poll_result(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<T, Error>>;

    /// Poll for task cancellation.
    ///
    /// This method attempts to cancel the task and returns [`Poll::Ready`] when
    /// the cancellation is complete.
    fn poll_cancel(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()>;

    /// Get the result of the task, including any errors that occurred.
    ///
    /// This is equivalent to awaiting the task but returns a [`Result`] that
    /// allows you to handle panics and other errors explicitly.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use executor_core::{Executor, Task};
    /// use executor_core::tokio::DefaultExecutor;
    ///
    /// let executor = DefaultExecutor::new();
    /// let task = executor.spawn(async { 42 });
    ///
    /// // Get result with explicit error handling
    /// match task.result().await {
    ///     Ok(value) => println!("Success: {}", value),
    ///     Err(error) => println!("Error: {:?}", error),
    /// }
    /// ```
    fn result(self) -> impl Future<Output = Result<T, Error>>
    where
        Self: Sized,
    {
        ResultFuture {
            task: self,
            _phantom: PhantomData,
        }
    }

    /// Cancel the task.
    ///
    /// This method requests cancellation of the task and returns a future that
    /// completes when the cancellation is finished.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use executor_core::{Executor, Task};
    /// use executor_core::tokio::DefaultExecutor;
    ///
    /// let executor = DefaultExecutor::new();
    /// let task = executor.spawn(async {
    ///     // Long running task
    ///     tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    ///     42
    /// });
    ///
    /// // Request task cancellation
    /// task.cancel().await;
    /// ```
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

/// A type-erased [`Executor`] that can hold any executor implementation.
///
/// This allows for runtime selection of executors and storing different executor
/// types in the same collection.
///
/// # Examples
///
/// ```ignore
/// use executor_core::{AnyExecutor, Executor};
/// use executor_core::tokio::DefaultExecutor;
///
/// let executor = DefaultExecutor::new();
/// let any_executor = AnyExecutor::new(executor);
///
/// let task = any_executor.spawn(async { "Hello from any executor!" });
/// let result = task.await;
/// ```
pub struct AnyExecutor(Box<dyn AnyExecutorImpl>);

/// Task type returned by [`AnyExecutor`].
///
/// This task can be awaited like any other task and provides the same
/// cancellation and error handling capabilities.
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
    /// # Examples
    ///
    /// ```ignore
    /// use executor_core::{init_local_executor, spawn_local};
    /// use executor_core::tokio::DefaultExecutor;
    ///
    /// init_local_executor(DefaultExecutor::new());
    ///
    /// let task = spawn_local(async { 42 });
    /// let result = task.await;
    /// ```
    pub fn init_local_executor(executor: impl LocalExecutor + 'static) {
        LOCAL_EXECUTOR.with(|cell| {
            cell.set(AnyLocalExecutor::new(executor))
                .expect("Local executor already set");
        });
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
    /// # Examples
    ///
    /// ```ignore
    /// use executor_core::{init_global_executor, spawn};
    /// use executor_core::tokio::DefaultExecutor;
    ///
    /// init_global_executor(DefaultExecutor::new());
    ///
    /// let task = spawn(async { 42 });
    /// let result = task.await;
    /// ```
    pub fn init_global_executor(executor: impl crate::Executor + 'static) {
        GLOBAL_EXECUTOR
            .set(AnyExecutor::new(executor))
            .expect("Global executor already set");
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
    /// # Examples
    ///
    /// ```ignore
    /// use executor_core::{init_global_executor, spawn};
    /// use executor_core::tokio::DefaultExecutor;
    ///
    /// init_global_executor(DefaultExecutor::new());
    ///
    /// let task = spawn(async {
    ///     println!("Hello from global executor!");
    ///     42
    /// });
    ///
    /// let result = task.await;
    /// ```
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
    /// # Examples
    ///
    /// ```ignore
    /// use executor_core::{init_local_executor, spawn_local};
    /// use std::rc::Rc;
    /// use executor_core::tokio::DefaultExecutor;
    ///
    /// init_local_executor(DefaultExecutor::new());
    ///
    /// let task = spawn_local(async {
    ///     // This future is not Send due to Rc
    ///     let local_data = Rc::new(42);
    ///     *local_data
    /// });
    ///
    /// let result = task.await;
    /// ```
    pub fn spawn_local<Fut>(fut: Fut) -> AnyLocalExecutorTask<Fut::Output>
    where
        Fut: Future + 'static,
    {
        LOCAL_EXECUTOR.with(|cell| {
            let executor = cell.get().expect("Local executor not set");
            executor.spawn(fut)
        })
    }

    #[allow(unused)]
    pub(crate) fn catch_unwind<F, R>(f: F) -> Result<R, Box<dyn std::any::Any + Send>>
    where
        F: FnOnce() -> R,
    {
        std::panic::catch_unwind(AssertUnwindSafe(f))
    }
}

pub use std_on::*;

// Re-export async-executor types
#[cfg(feature = "async-executor")]
#[cfg_attr(docsrs, doc(cfg(feature = "async-executor")))]
pub use async_executor::AsyncTask;

// Re-export tokio types
#[cfg(feature = "tokio")]
#[cfg_attr(docsrs, doc(cfg(feature = "tokio")))]
pub use tokio::{DefaultExecutor, TokioLocalTask, TokioTask};

// Re-export web types
#[cfg(feature = "web")]
#[cfg_attr(docsrs, doc(cfg(feature = "web")))]
pub use web::{WebExecutor, WebTask};
