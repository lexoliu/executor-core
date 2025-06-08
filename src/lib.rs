#![no_std]
//! A flexible task executor abstraction layer for Rust async runtimes.
//!
//! This crate provides a unified interface for spawning and managing async tasks across
//! different executor backends. It supports both global and local executors, with built-in
//! implementations for popular runtimes like [`async-executor`] and [`tokio`].

//! # Quick Start
//!
//! ```rust
//! use executor_core::spawn;
//!
//! // Spawn a task on the global executor
//! let task = spawn(async {
//!     println!("Hello from async task!");
//!     42
//! });
//!
//! // Await the result
//! let result = task.await;
//! assert_eq!(result, 42);
//! ```
//!
//! # Error Handling
//!
//! Tasks can be awaited directly (panics propagate) or handled explicitly:
//!
//! ```rust
//! use executor_core::{spawn, Error};
//! let task = spawn(async {
//!     panic!("Something went wrong");
//!     42
//! });
//!
//! // Option 1: Direct await (panics propagate)
//! let result = task.await;
//!
//! // Option 2: Explicit error handling
//! match task.result().await {
//!     Ok(value) => println!("Success: {}", value),
//!     Err(Error::Panicked(msg)) => println!("Task panicked: {}", msg),
//!     Err(Error::Cancelled) => println!("Task was cancelled"),
//! }
//!
//! ```
//!
//! # Thread Safety
//!
//! The crate provides separate traits for thread-safe and thread-local execution:
//!
//! - [`Executor`] + [`Task`]: For `Send` futures that can move between threads
//! - [`LocalExecutor`] + [`LocalTask`]: For non-`Send` futures bound to one thread
//!
//! ```rust
//! use executor_core::{Executor, LocalExecutor};
//! use std::rc::Rc;
//!
//! // ✅ Send futures work with Executor
//! let executor = async_executor::Executor::new();
//! let task = executor.spawn(async { "Hello".to_string() });
//!
//! // ✅ Non-Send futures work with LocalExecutor
//! let local_executor = async_executor::LocalExecutor::new();
//! let local_task = local_executor.spawn(async {
//!     let data = Rc::new(42); // Rc is not Send
//!     *data
//! });
//! ```
//!
//! # Feature Flags
//!
//! - `default-async-executor` (default) - Use [`async-executor`] as the global executor
//! - `default-tokio` - Use [`tokio`] as the global executor
//! - `async-executor` - Enable [`async-executor`] backend support
//! - `tokio` - Enable [`tokio`] backend support
//! - `std` - Enable standard library support (auto-enabled by executor backends)
//!
//! [`async-executor`]: https://docs.rs/async-executor
//! [`tokio`]: https://docs.rs/tokio

extern crate alloc;
use alloc::borrow::Cow;
use core::future::Future;

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "async-executor")]
mod async_executor;
#[cfg(feature = "tokio")]
mod tokio;

#[cfg(feature = "default-async-executor")]
#[doc(hidden)]
pub use async_executor::DefaultExecutor;

#[cfg(feature = "default-async-executor")]
#[doc(hidden)]
pub use async_executor::DefaultLocalExecutor;

#[cfg(all(feature = "default-async-executor", feature = "default-tokio"))]
compile_error!(
    "Cannot enable both default-async-executor and default-tokio features simultaneously"
);

/// A trait for executor implementations that can spawn thread-safe futures.
///
/// This trait represents executors capable of running concurrent tasks that are `Send`,
/// meaning they can be safely moved between threads. Implementors provide the core
/// functionality for task spawning in a multi-threaded context.
pub trait Executor {
    /// Spawns a thread-safe future on this executor.
    fn spawn<T: Send + 'static>(
        &self,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> impl Task<Output = T>;
}

/// A trait for executor implementations that can spawn futures on the current thread.
///
/// This trait represents executors that operate within a single thread context,
/// allowing them to work with futures that are not `Send`. This is essential for
/// working with non-thread-safe types like [Rc](alloc::rc::Rc), [RefCell](core::cell::RefCell), or thread-local storage.
pub trait LocalExecutor {
    /// Spawns a future on this local executor.
    fn spawn<T: 'static>(
        &self,
        fut: impl Future<Output = T> + 'static,
    ) -> impl LocalTask<Output = T>;
}

/// A handle to a spawned task that can be safely moved between threads.
///
/// This trait extends [`Future`], which means it can be awaited to get the result directly.
///
/// # Panics
/// If the task is cancelled or panics during execution, the attempt to await the task will panic.
/// If you need to handle these cases gracefully, use the [`result`] method instead.
pub trait Task: Future + 'static + Send {
    /// Returns the task result or error without panicking.
    ///
    /// Unlike directly awaiting the task, this method returns a [`Result`] that
    /// allows you to handle task panics and cancellation gracefully.
    ///
    /// # Returns
    ///
    /// - `Ok(T)` - Task completed successfully
    /// - `Err(Error::Panicked(_))` - Task panicked during execution
    /// - `Err(Error::Cancelled)` - Task was cancelled
    ///
    /// # Examples
    ///
    /// ```rust
    /// use executor_core::{spawn, Error};
    ///
    /// let task = spawn(async { 42 });
    ///
    /// match task.result().await {
    ///     Ok(value) => println!("Result: {}", value),
    ///     Err(Error::Panicked(msg)) => eprintln!("Panic: {}", msg),
    ///     Err(Error::Cancelled) => println!("Cancelled"),
    /// }
    /// ```
    fn result(self) -> impl Future<Output = Result<Self::Output, Error>> + Send;

    /// Cancels the task, preventing further execution.
    ///
    /// After calling this method, the task will stop executing as soon as possible.
    /// Any attempt to await the task or get its result will return [`Error::Cancelled`].
    ///
    /// # Notes
    ///
    /// - Cancellation is cooperative and may not be immediate
    /// - Already completed tasks cannot be cancelled
    ///
    /// # Examples
    ///
    /// ```rust
    /// use executor_core::spawn;
    /// use std::time::Duration;
    ///
    /// let task = spawn(async {
    ///     tokio::time::sleep(Duration::from_secs(10)).await;
    ///     "Done"
    /// });
    ///
    /// // Cancel the long-running task
    /// task.cancel();
    /// ```
    fn cancel(self);
}

/// A handle to a spawned task that may not be `Send`.
///
/// Similar to [`Task`], but for futures that must execute on the same thread
/// they were spawned on. This is useful for working with non-thread-safe types.
pub trait LocalTask: Future + 'static {
    /// Returns the task result or error without panicking.
    ///
    /// Similar to [`Task::result`], but for local tasks that are not `Send`.
    fn result(self) -> impl Future<Output = Result<Self::Output, Error>>;

    /// Cancels the local task, preventing further execution.
    ///
    /// Similar to [`Task::cancel`], but for local tasks.
    fn cancel(self);
}

/// Errors that can occur during task execution.
///
/// This enum represents the different ways a task can fail, allowing for
/// proper error handling and recovery strategies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The task panicked during execution.
    ///
    /// Contains the panic message if available, or a generic message if the
    /// panic payload couldn't be converted to a string.
    Panicked(Cow<'static, str>),

    /// The task was cancelled before completion.
    ///
    /// This occurs when [`Task::cancel`] or [`LocalTask::cancel`] is called
    /// on a running task.
    Cancelled,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::Panicked(msg) => write!(f, "Task panicked: {msg}"),
            Error::Cancelled => write!(f, "Task was cancelled"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

#[cfg(feature = "std")]
static GLOBAL_EXECUTOR: std::sync::LazyLock<DefaultExecutor> =
    std::sync::LazyLock::new(DefaultExecutor::default);

/// Spawns a new task on the global executor.
///
/// This is a convenience function that spawns a task using the default global executor
/// instance. The spawned task must be `Send + 'static`, allowing it to be moved between
/// threads.
///
/// # Global Executor
///
/// The global executor is determined by feature flags:
/// - With `default-async-executor` (default): Uses [`async_executor::Executor`]
/// - With `default-tokio`: Uses [`tokio::runtime::Runtime`]
#[cfg(feature = "std")]
pub fn spawn<T: Send + 'static>(
    fut: impl Future<Output = T> + Send + 'static,
) -> impl Task<Output = T> {
    use core::ops::Deref;
    Executor::spawn(GLOBAL_EXECUTOR.deref(), fut)
}
