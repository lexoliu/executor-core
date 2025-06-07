#![no_std]
//! A flexible task executor abstraction layer for Rust async runtimes.
//!
//! This crate provides a unified interface for spawning and managing async tasks across
//! different executor backends. It supports both global and local executors, with built-in
//! implementations for popular runtimes like `async-executor` and `tokio`.
//!
//! # Features
//!
//! - Abstract interface for spawning async tasks
//! - Support for multiple executor backends
//! - Global and local executor variants
//! - Zero-cost abstractions
//! - Simple, lightweight API
//!
//! # Examples
//!
//! ```rust
//! use executor_core::{spawn, Task};
//!
//! async fn example() {
//!     // Spawn a task on the global executor
//!     let task = spawn(async {
//!         println!("Hello from task!");
//!         42
//!     });
//!
//!     // Wait for the task result
//!     let result = task.await;
//!     assert_eq!(result, 42);
//! }
//! ```
//!
//! # Feature Flags
//!
//! - `default-async-executor` (default) - Use async-executor as the default implementation
//! - `default-tokio` - Use tokio as the default implementation
//! - `async-executor` - Enable async-executor support
//! - `tokio` - Enable tokio support

extern crate alloc;
use alloc::borrow::Cow;
use core::ops::Deref;
#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "async-executor")]
mod async_executor;
#[cfg(feature = "tokio")]
mod tokio;

#[cfg(feature = "default-async-executor")]
pub type DefaultExecutor = ::async_executor::Executor<'static>;

#[cfg(feature = "default-async-executor")]
pub type DefaultLocalExecutor = ::async_executor::LocalExecutor<'static>;

/// A trait for executor implementations that can spawn `Send` futures.
///
/// This trait represents executors capable of running concurrent tasks that are `Send`,
/// meaning they can be safely moved between threads. Implementors of this trait
/// provide the core functionality for task spawning in a multi-threaded context.
///
/// # Examples
///
/// ```rust
/// use executor_core::{Executor, Task};
///
/// struct MyExecutor;
///
/// impl Executor for MyExecutor {
///     fn spawn<T: Send + 'static>(
///         &self,
///         fut: impl Future<Output = T> + Send + 'static,
///     ) -> impl Task<Output = T> {
///         // Implementation details...
///     }
/// }
/// ```
pub trait Executor {
    fn spawn<T: Send + 'static>(
        &self,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> impl Task<Output = T>;
}

/// A trait for executor implementations that can spawn non-`Send` futures.
///
/// This trait represents executors that operate within a single thread context,
/// allowing them to work with futures that are not `Send`. This is useful for
/// cases where you need to work with non-thread-safe types or maintain thread-local state.
///
/// # Examples
///
/// ```rust
/// use executor_core::{LocalExecutor, LocalTask};
///
/// struct MyLocalExecutor;
///
/// impl LocalExecutor for MyLocalExecutor {
///     fn spawn<T: 'static>(
///         &self,
///         fut: impl Future<Output = T> + 'static,
///     ) -> impl LocalTask<Output = T> {
///         // Implementation details...
///     }
/// }
/// ```
pub trait LocalExecutor {
    fn spawn<T: 'static>(
        &self,
        fut: impl Future<Output = T> + 'static,
    ) -> impl LocalTask<Output = T>;
}

/// A trait representing a spawned task that is `Send`.
///
/// This trait extends `Future` and provides additional functionality for handling
/// task results and cancellation. Tasks implementing this trait can be safely
/// moved between threads and represent the result of spawning a future on a
/// multi-threaded executor.
///
/// # Methods
///
/// - `result`: Returns a future that resolves to either the task's result or an error
/// - `cancel`: Cancels the task, preventing further execution
///
/// # Examples
///
/// ```rust
/// use executor_core::{Task, Error};
///
/// async fn handle_task<T: Task>(task: T) {
///     match task.result().await {
///         Ok(value) => println!("Task completed: {:?}", value),
///         Err(Error::Cancelled) => println!("Task was cancelled"),
///         Err(Error::Panicked(_)) => println!("Task panicked"),
///     }
/// }
/// ```
pub trait Task: Future + 'static + Send {
    fn result(self) -> impl Future<Output = Result<Self::Output, Error>> + Send;
    fn cancel(self);
}

/// A trait representing a spawned task that may not be `Send`.
///
/// Similar to the `Task` trait, but for futures that must execute on the same thread
/// they were spawned on. This is useful for working with non-thread-safe types or
/// maintaining thread-local state.
///
/// # Methods
///
/// - `result`: Returns a future that resolves to either the task's result or an error
/// - `cancel`: Cancels the task, preventing further execution
///
/// # Examples
///
/// ```rust
/// use executor_core::{LocalTask, Error};
///
/// async fn handle_local_task<T: LocalTask>(task: T) {
///     match task.result().await {
///         Ok(value) => println!("Task completed: {:?}", value),
///         Err(Error::Cancelled) => println!("Task was cancelled"),
///         Err(Error::Panicked(_)) => println!("Task panicked"),
///     }
/// }
/// ```
pub trait LocalTask: Future + 'static {
    fn result(self) -> impl Future<Output = Result<Self::Output, Error>>;
    fn cancel(self);
}

/// Errors that can occur during task execution.
#[derive(Debug)]
pub enum Error {
    /// The task panicked during execution. Contains the panic message if available.
    Panicked(Cow<'static, str>),
    /// The task was cancelled before completion.
    Cancelled,
}

static GLOBAL_EXECUTOR: std::sync::LazyLock<DefaultExecutor> =
    std::sync::LazyLock::new(DefaultExecutor::default);

/// Spawns a new task on the global executor.
///
/// This is a convenience function that spawns a task using the default global executor
/// instance. The spawned task must be `Send`, allowing it to be moved between threads.
///
/// # Examples
///
/// ```rust
/// use executor_core::spawn;
///
/// async fn example() {
///     let task = spawn(async {
///         // Some async work
///         42
///     });
///
///     let result = task.await;
///     assert_eq!(result, 42);
/// }
/// ```
pub fn spawn<T: Send + 'static>(
    fut: impl Future<Output = T> + Send + 'static,
) -> impl Task<Output = T> {
    Executor::spawn(GLOBAL_EXECUTOR.deref(), fut)
}
