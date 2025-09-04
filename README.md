# executor-core

[![MIT licensed][mit-badge]][mit-url]
[![Crates.io][crates-badge]][crates-url]
[![Documentation][docs-badge]][docs-url]

[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: LICENSE
[crates-badge]: https://img.shields.io/crates/v/executor-core.svg
[crates-url]: https://crates.io/crates/executor-core
[docs-badge]: https://docs.rs/executor-core/badge.svg
[docs-url]: https://docs.rs/executor-core

A flexible task executor abstraction layer for Rust async runtimes.

## Overview

`executor-core` provides a unified interface for spawning and managing async tasks across different executor backends. Write once, run on any supported async runtime.

## Features

- **Runtime agnostic** - Works with `async-executor`, `tokio`, web (WASM), and custom executors
- **Unified task interface** - Uses `async_task::Task` for all backends
- **Zero-cost abstractions** - Compiles to direct executor calls
- **No-std support** - Works in embedded environments

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
executor-core = "0.1"
```

Basic usage:

```rust
use executor_core::Executor;
use async_executor::Executor as AsyncExecutor;

#[tokio::main]
async fn main() {
    let executor = AsyncExecutor::new();
    let task = executor.spawn(async {
        println!("Hello from task!");
        42
    });

    let result = task.await;
    println!("Result: {}", result);
}
```


## Task Management

```rust
use executor_core::Executor;

// Spawn and await
let task = executor.spawn(async { 42 });
let result = task.await;

// Cancel a task
let task = executor.spawn(async { /* long running */ });
task.cancel().await;

// Detach to run in background
let task = executor.spawn(async { /* background work */ });
task.detach();
```

## Feature Flags

- `async-executor` - Enable `async-executor` backend
- `tokio` - Enable `tokio` backend  
- `web` - Enable web backend support (WASM)
- `std` - Enable standard library support
## License

Licensed under the [MIT License](LICENSE).
