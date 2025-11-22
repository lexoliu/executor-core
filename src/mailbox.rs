//! Mailbox-based message passing for safe cross-thread communication.
//!
//! This module provides a [`Mailbox`] type that enables asynchronous message passing
//! to a value owned by a background task. The mailbox ensures thread-safe access
//! to the contained value by serializing all operations through a message queue.
//!
//! # Overview
//!
//! The mailbox pattern is useful when you need to:
//! - Share mutable state across threads safely
//! - Process operations on a value sequentially
//! - Avoid blocking when sending updates
//! - Make async calls that return values
//!
//! # Examples
//!
//! ```rust
//! # async fn docs() {
//! use executor_core::mailbox::Mailbox;
//! use std::{cell::RefCell, collections::HashMap};
//!
//! // Create a mailbox containing a HashMap on the main executor
//! let mailbox = Mailbox::main(RefCell::new(HashMap::<String, i32>::new()));
//!
//! // Send updates to the value (non-blocking)
//! mailbox.handle(|map| {
//!     map.borrow_mut().insert("key".to_string(), 42);
//! });
//!
//! // Make async calls that return values
//! let value = mailbox.call(|map| {
//!     map.borrow().get("key").copied().unwrap_or(0)
//! }).await;
//! # let _ = value;
//! # }
//! ```

use crate::LocalExecutor;
use alloc::boxed::Box;
use async_channel::{Sender, unbounded};

type Job<T> = Box<dyn Send + FnOnce(&T)>;

/// A mailbox for sending messages to a value owned by a background task.
///
/// `Mailbox<T>` provides thread-safe access to a value of type `T` by serializing
/// all operations through an async message queue. The value is owned by a background
/// task that processes incoming messages sequentially.
///
/// # Type Parameters
///
/// * `T` - The type of value contained in the mailbox. Must be `'static` to ensure
///   the background task can own it safely.
///
/// # Thread Safety
///
/// The mailbox enables other threads to safely access a value living on another thread
/// without explicit locks. The mailbox handle itself is always `Send` and `Sync`,
/// allowing it to be shared across threads. All operations on the value are serialized
/// through an async message queue, providing lock-free concurrent access. When `T` is
/// not `Send`, the value remains pinned to its original thread but can still be safely
/// accessed from other threads through the mailbox.
#[derive(Debug)]
pub struct Mailbox<T: 'static> {
    sender: Sender<Job<T>>,
}

impl<T: 'static> Mailbox<T> {
    /// Creates a new mailbox with the given value on the specified executor.
    ///
    /// The value will be moved to a background task that processes incoming
    /// messages. The executor is consumed to spawn the background task.
    ///
    /// # Parameters
    ///
    /// * `executor` - The executor to spawn the background task on
    /// * `value` - The value to be owned by the background task
    ///
    /// # Examples
    ///
    /// ```rust
    /// use executor_core::{mailbox::Mailbox, NativeExecutor};
    /// use std::{cell::RefCell, collections::HashMap};
    ///
    /// let mailbox = Mailbox::new(NativeExecutor, RefCell::new(HashMap::<String, i32>::new()));
    /// # let _ = mailbox;
    /// ```
    #[allow(clippy::needless_pass_by_value)]
    pub fn new<E: LocalExecutor>(executor: E, value: T) -> Self {
        let (sender, receiver) = unbounded::<Box<dyn Send + FnOnce(&T)>>();

        let _fut = executor.spawn_local(async move {
            while let Ok(update) = receiver.recv().await {
                update(&value);
            }
        });
        Self { sender }
    }

    /// Sends a non-blocking update to the mailbox value.
    ///
    /// The provided closure will be called with a reference to the value
    /// in the background task. This operation is non-blocking and will
    /// not wait for the update to be processed.
    ///
    /// If the background task has been dropped or the channel is full,
    /// the update may be silently discarded.
    ///
    /// # Parameters
    ///
    /// * `update` - A closure that will be called with a reference to the value
    ///
    /// # Examples
    ///
    /// ```rust
    /// use executor_core::mailbox::Mailbox;
    /// use std::{cell::RefCell, collections::HashMap};
    ///
    /// let mailbox = Mailbox::main(RefCell::new(HashMap::<String, i32>::new()));
    ///
    /// // Send a non-blocking update
    /// mailbox.handle(|map| {
    ///     map.borrow_mut().insert("key".to_string(), 42);
    /// });
    /// ```
    pub fn handle(&self, update: impl FnOnce(&T) + Send + 'static) {
        let _ = self.sender.try_send(Box::new(update));
    }

    /// Makes an asynchronous call to the mailbox value and returns the result.
    ///
    /// The provided closure will be called with a reference to the value
    /// in the background task, and the result will be returned to the caller.
    /// This operation blocks until the call is processed and the result is available.
    ///
    /// # Parameters
    ///
    /// * `f` - A closure that will be called with a reference to the value and returns a result
    ///
    /// # Returns
    ///
    /// The result returned by the closure after it has been executed on the value.
    ///
    /// # Panics
    ///
    /// Panics if the background task has been dropped or the channel is closed,
    /// making it impossible to receive the result.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # async fn example() {
    /// use executor_core::mailbox::Mailbox;
    /// use std::{cell::RefCell, collections::HashMap};
    ///
    /// let mailbox = Mailbox::main(RefCell::new(HashMap::<String, i32>::new()));
    ///
    /// // Make an async call that returns a value
    /// let value = mailbox.call(|map| {
    ///     map.borrow().get("key").copied().unwrap_or(0)
    /// }).await;
    /// # let _ = value;
    /// # }
    /// ```
    pub async fn call<R>(&self, f: impl FnOnce(&T) -> R + Send + 'static) -> R
    where
        R: Send + 'static,
    {
        let (s, r) = async_channel::bounded(1);
        self.handle(move |v| {
            let res = f(v);
            let _ = s.try_send(res);
        });
        r.recv().await.expect("Mailbox call failed")
    }
}
