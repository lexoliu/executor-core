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

- **Runtime agnostic** - Works with `async-executor`, `tokio`, and custom executors
- **Zero-cost abstractions** - Compiles to direct executor calls
- **Panic handling** - Graceful error recovery from task panics
- **No-std support** - Works in embedded environments

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
executor-core = "0.1"
```

Basic usage:

```rust
use executor_core::spawn;

async fn main() {
    let task = spawn(async {
        println!("Hello from task!");
        42
    });

    let result = task.await;
    println!("Result: {}", result);
}
```


## Error Handling

```rust
use executor_core::{spawn, Error};

let task = spawn(async { 42 });

match task.result().await {
    Ok(value) => println!("Success: {}", value),
    Err(Error::Panicked(msg)) => println!("Task panicked: {}", msg),
    Err(Error::Cancelled) => println!("Task was cancelled"),
}
```

## Feature Flags

- `default-async-executor` (default) - Use `async-executor` as global executor
- `default-tokio` - Use `tokio` as global executor
- `async-executor` - Enable `async-executor` backend
- `tokio` - Enable `tokio` backend
- `std` - Enable standard library support
## License

Licensed under the [MIT License](LICENSE).
