# executor-core

[![MIT licensed][mit-badge]][mit-url]
[![Crates.io][crates-badge]][crates-url]

[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: LICENSE
[crates-badge]: https://img.shields.io/crates/v/executor-core.svg
[crates-url]: https://crates.io/crates/executor-core

A flexible task executor abstraction layer for Rust async runtimes.

## Overview

`executor-core` provides a unified interface for spawning and managing async tasks across different executor backends. It's designed to be lightweight, extensible, and efficient, making it easy to write runtime-agnostic async code.

## Features

- Abstract interface for spawning async tasks
- Support for multiple executor backends:
  - [async-executor](https://crates.io/crates/async-executor) (default)
  - [tokio](https://crates.io/crates/tokio)
- Global and local executor variants
- Zero-cost abstractions
- Simple, lightweight API
- #![no_std] support (with std feature)
- Panic handling and task cancellation

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
executor-core = "0.1"
```

### Quick Start

```rust
use executor_core::{spawn, Task};

async fn example() {
    // Spawn a task on the global executor
    let task = spawn(async {
        println!("Hello from task!");
        42
    });

    // Wait for the task result
    let result = task.await;
    assert_eq!(result, 42);
}
```

### Using Explicit Executors

```rust
use executor_core::{Executor, Task};
use async_executor::Executor as AsyncExecutor;

async fn example_with_executor() {
    let executor = AsyncExecutor::new();

    let task = executor.spawn(async {
        // Some async work
        "Done!"
    });

    let result = task.await;
    assert_eq!(result, "Done!");
}
```

### Error Handling

```rust
use executor_core::{spawn, Task, Error};

async fn example_with_error_handling() {
    let task = spawn(async {
        // Some potentially failing work
        42
    });

    match task.result().await {
        Ok(value) => println!("Task completed: {}", value),
        Err(Error::Cancelled) => println!("Task was cancelled"),
        Err(Error::Panicked(msg)) => println!("Task panicked: {}", msg),
    }
}
```

## Feature Flags

- `default-async-executor` (default) - Use async-executor as the default implementation
- `default-tokio` - Use tokio as the default implementation
- `async-executor` - Enable async-executor support
- `tokio` - Enable tokio support
- `std` - Enable standard library support (enabled by executor backends)

## Architecture

The crate provides these key traits:

- `Executor` - For spawning `Send` futures that can run on any thread
- `LocalExecutor` - For spawning non-`Send` futures that must run on the spawning thread
- `Task` - Common interface for spawned tasks that are `Send`
- `LocalTask` - Common interface for spawned tasks that are not `Send`

And concrete implementations:

- Global executor accessible via `spawn()`
- Explicit executor instances via supported backend implementations
- Error handling for task panics and cancellation

## Implementation

To implement a custom executor:

```rust
use executor_core::{Executor, Task};
use std::future::Future;

struct MyExecutor;

impl Executor for MyExecutor {
    fn spawn<T: Send + 'static>(
        &self,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> impl Task<Output = T> {
        // Implementation details...
    }
}
```

## Performance

The abstractions in this crate are designed to be zero-cost - they should compile away to the same code you'd get from using the underlying executors directly. The trait-based design ensures that the compiler can inline and optimize across trait boundaries.

## Thread Safety

Tasks spawned via `Executor` must be `Send`, allowing them to be moved between threads. For tasks that must remain on their spawning thread, use `LocalExecutor` instead. This separation helps prevent runtime errors and makes thread-safety requirements explicit in the type system.

## License

This project is licensed under the [MIT license](LICENSE).
