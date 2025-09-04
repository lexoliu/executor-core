#![no_std]
//! Write async libraries without choosing a runtime.
//!
//! Your users should decide whether to use tokio, async-std, or any other runtime.
//! Not you.
//!
//! # How It Works
//!
//! Instead of hard-coding `tokio::spawn`, accept an executor parameter:
//!
//! ```rust
//! use executor_core::Executor;
//!
//! pub async fn parallel_sum<E: Executor>(
//!     executor: &E,
//!     numbers: Vec<i32>
//! ) -> i32 {
//!     let (left, right) = numbers.split_at(numbers.len() / 2);
//!     
//!     // Spawn on ANY runtime via the executor parameter
//!     let left_sum = executor.spawn(async move {
//!         left.iter().sum::<i32>()
//!     });
//!     
//!     let right_sum = executor.spawn(async move {
//!         right.iter().sum::<i32>()
//!     });
//!     
//!     left_sum.await + right_sum.await
//! }
//! ```
//!
//! Now users can call your library with their preferred runtime:
//!
//! ```rust,no_run
//! # use executor_core::Executor;
//! # async fn parallel_sum<E: Executor>(executor: &E, numbers: Vec<i32>) -> i32 { 0 }
//!
//! // User already using tokio? Great!
//! # #[cfg(feature = "tokio")]
//! tokio::runtime::Runtime::new().unwrap().block_on(async {
//!     let runtime = tokio::runtime::Handle::current();
//!     let sum = parallel_sum(&runtime, vec![1, 2, 3, 4]).await;
//! });
//!
//! // User prefers async-executor? Also great!
//! # #[cfg(feature = "async-executor")]  
//! async_executor::Executor::new().run(async {
//!     let executor = async_executor::Executor::new();
//!     let sum = parallel_sum(&executor, vec![1, 2, 3, 4]).await;
//! });
//! ```
//!
//! # Quick Start
//!
//! **For library authors:** Just add the core crate, no features needed.
//! ```toml
//! [dependencies]
//! executor-core = "0.2"
//! ```
//!
//! **For app developers:** Add with your runtime's feature.
//! ```toml
//! [dependencies]
//! executor-core = { version = "0.2", features = ["tokio"] }
//! ```
//!
//! # API
//!
//! Two traits:
//! - [`Executor`] - For `Send` futures
//! - [`LocalExecutor`] - For non-`Send` futures (Rc, RefCell, etc.)
//!
//! Both return [`async_task::Task`]:
//! ```rust,ignore
//! let task = executor.spawn(async { work() });
//! let result = task.await;        // Get result
//! task.cancel().await;            // Cancel task
//! task.detach();                  // Run in background
//! ```
//!
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

pub use async_task::*;
