#![no_std]
//! A flexible task executor abstraction layer for Rust async runtimes.
//!
//! This crate provides a unified interface for spawning and managing async tasks across
//! different executor backends. It supports both global and local executors, with built-in
//! implementations for popular runtimes like [`async-executor`] and [`tokio`].
//!
//! # Quick Start
//!
//! ```rust
//! use executor_core::Executor;
//! use async_executor::Executor as AsyncExecutor;
//!
//! let executor = AsyncExecutor::new();
//! // Spawn a task on the executor
//! let task = executor.spawn(async {
//!     println!("Hello from async task!");
//!     42
//! });
//!
//! // Await the result
//! let result = futures_lite::future::block_on(task);
//! assert_eq!(result, 42);
//! ```
//!
//! # Thread Safety
//!
//! The crate provides separate traits for thread-safe and thread-local execution:
//!
//! - [`Executor`]: For `Send` futures that can move between threads
//! - [`LocalExecutor`]: For non-`Send` futures bound to one thread
//!
//! Both traits return [`async_task::Task`] which provides:
//! - `await` to get the result
//! - `cancel()` to cancel the task
//! - `detach()` to run the task in background
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
//! - `async-executor` - Enable [`async-executor`] backend support
//! - `tokio` - Enable [`tokio`] backend support
//! - `web` - Enable web backend support with [`wasm-bindgen-futures`]
//! - `std` - Enable standard library support (auto-enabled by executor backends)
//!
//! [`async-executor`]: https://docs.rs/async-executor
//! [`tokio`]: https://docs.rs/tokio
//! [`wasm-bindgen-futures`]: https://docs.rs/wasm-bindgen-futures

extern crate alloc;
use core::future::Future;

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "async-executor")]
mod async_executor;
#[cfg(feature = "tokio")]
mod tokio;

#[cfg(feature = "web")]
pub mod web;

/// A trait for executor implementations that can spawn thread-safe futures.
///
/// This trait represents executors capable of running concurrent tasks that are `Send`,
/// meaning they can be safely moved between threads. Implementors provide the core
/// functionality for task spawning in a multi-threaded context.
pub trait Executor {
    /// Spawns a thread-safe future on this executor.
    ///
    /// Returns an [`async_task::Task`] handle that can be used to:
    /// - Await the result
    /// - Cancel the task
    /// - Detach the task to run in background
    fn spawn<T: Send + 'static>(
        &self,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> async_task::Task<T>;
}

/// A trait for executor implementations that can spawn futures on the current thread.
///
/// This trait represents executors that operate within a single thread context,
/// allowing them to work with futures that are not `Send`. This is essential for
/// working with non-thread-safe types like [Rc](alloc::rc::Rc), [RefCell](core::cell::RefCell), or thread-local storage.
pub trait LocalExecutor {
    /// Spawns a future on this local executor.
    ///
    /// Returns an [`async_task::Task`] handle that can be used to:
    /// - Await the result
    /// - Cancel the task
    /// - Detach the task to run in background
    fn spawn<T: 'static>(&self, fut: impl Future<Output = T> + 'static) -> async_task::Task<T>;
}
