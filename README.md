# executor-core

A flexible task executor abstraction layer for Rust async runtimes.

[![Crates.io](https://img.shields.io/crates/v/executor-core.svg)](https://crates.io/crates/executor-core)
[![Documentation](https://docs.rs/executor-core/badge.svg)](https://docs.rs/executor-core)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Overview

`executor-core` provides unified traits and type-erased wrappers for different async executors in Rust. It allows you to write code that's agnostic to the underlying executor implementation, whether you're using Tokio, async-executor, or custom executors.

Write async libraries without choosing a runtime. Your users should decide whether to use tokio, async-executor, or any other runtime. Not you.

## Features

- **Zero-cost Executor Abstraction**: Unified `Executor` and `LocalExecutor` traits, using GAT to prevent unnecessary heap allocation and dynamic dispatch.

- **Type Erasure**: `AnyExecutor` and `AnyLocalExecutor` for runtime flexibility
- **Multiple Runtime Support**:
  - **Tokio**: Integration with Tokio runtime and LocalSet
  - **async-executor**: Support for async-executor crate
  - **Web/WASM**: Browser-compatible executor for web applications
- **Task Management**: Rich task API with cancellation and error handling
- **No-std Compatible**: Core functionality works in no-std environments
- **Panic Safety**: Proper panic handling and propagation

## How It Works

Instead of hard-coding `tokio::spawn`, accept an executor parameter:

```rust
use executor_core::Executor;

pub async fn parallel_sum<E: Executor>(
    executor: &E,
    numbers: Vec<i32>
) -> i32 {
    let (left, right) = numbers.split_at(numbers.len() / 2);

    let left_sum = executor.spawn(async move {
        left.iter().sum::<i32>()
    });

    let right_sum = executor.spawn(async move {
        right.iter().sum::<i32>()
    });

    left_sum.await + right_sum.await
}
```

Users call it with their runtime:

```rust
// tokio users
let runtime = tokio::runtime::Runtime::new()?;
let sum = parallel_sum(&runtime, vec![1, 2, 3, 4]).await;

// async-executor users
let executor = async_executor::Executor::new();
let sum = parallel_sum(&executor, vec![1, 2, 3, 4]).await;
```

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
executor-core = "0.3"
```

### Basic Usage

```rust
use executor_core::{Executor, init_global_executor, spawn};
use executor_core::tokio::DefaultExecutor;

#[tokio::main]
async fn main() {
    // Initialize the global executor
    init_global_executor(DefaultExecutor::new());

    // Spawn a task
    let task = spawn(async {
        println!("Hello from spawned task!");
        42
    });

    let result = task.await;
    println!("Task result: {}", result);
}
```

### Using Different Executors

```rust
use executor_core::{Executor, AnyExecutor};

// Tokio executor
let tokio_executor = executor_core::tokio::DefaultExecutor::new();
let task = tokio_executor.spawn(async { "tokio result" });

// Type-erased executor
let any_executor = AnyExecutor::new(tokio_executor);
let task = any_executor.spawn(async { "any executor result" });
```

### Local Executors (Non-Send Futures)

```rust
use executor_core::{LocalExecutor, init_local_executor, spawn_local};
use executor_core::tokio::DefaultExecutor;

#[tokio::main]
async fn main() {
    // Initialize local executor
    init_local_executor(DefaultExecutor::new());

    let task = spawn_local(async {
        // This future doesn't need to be Send
        let local_data = std::rc::Rc::new(42);
        *local_data
    });

    let result = task.await;
    println!("Local task result: {}", result);
}
```

### Task Cancellation

```rust
use executor_core::{Executor, Task};

let executor = executor_core::tokio::DefaultExecutor::new();
let task = executor.spawn(async {
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    "completed"
});

// Cancel the task
task.cancel().await;
```

### Error Handling

```rust
use executor_core::{Executor, Task};

let executor = executor_core::tokio::DefaultExecutor::new();
let task = executor.spawn(async {
    panic!("Something went wrong!");
});

// Handle task result with error
match task.result().await {
    Ok(value) => println!("Task completed: {}", value),
    Err(error) => println!("Task failed: {:?}", error),
}
```

## Runtime Support

### Tokio

```toml
[dependencies]
executor-core = { version = "0.3", features = ["tokio"] }
```

```rust
use executor_core::tokio::{DefaultExecutor, TokioTask, TokioLocalTask};

// Global executor
let executor = DefaultExecutor::new();

// Or use Tokio runtime directly
let runtime = tokio::runtime::Runtime::new().unwrap();
let task = runtime.spawn(async { "direct runtime usage" });
```

### async-executor

```toml
[dependencies]
executor-core = { version = "0.3", features = ["async-executor"] }
```

```rust
use executor_core::AsyncTask;

let executor = async_executor::Executor::new();
let task: AsyncTask<_> = executor.spawn(async { "async-executor" });
```

### Web/WASM

```toml
[dependencies]
executor-core = { version = "0.3", features = ["web"] }
```

```rust
use executor_core::web::WebExecutor;

let executor = WebExecutor::new();
let task = executor.spawn(async { "web task" });
```

## Feature Flags

- `std` - Enable std functionality (enabled by default)
- `tokio` - Tokio runtime support (enabled by default)
- `async-executor` - async-executor support (enabled by default)
- `web` - Web/WASM support (enabled by default)
- `full` - Enable all features

## Architecture

The crate is built around two main traits:

- **`Executor`**: For spawning `Send + 'static` futures
- **`LocalExecutor`**: For spawning `'static` futures (not necessarily `Send`)

Both traits produce tasks that implement the `Task` trait, providing:

- `Future` implementation for awaiting results
- `poll_result()` for explicit error handling
- `poll_cancel()` for task cancellation

Type-erased versions (`AnyExecutor`, `AnyLocalExecutor`) allow runtime executor selection.

## No-std Support

Core functionality works in `no-std` environments:

```toml
[dependencies]
executor-core = { version = "0.3", default-features = false }
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
