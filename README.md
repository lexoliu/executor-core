# executor-core

[![MIT licensed][mit-badge]][mit-url]
[![Crates.io][crates-badge]][crates-url]

[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: LICENSE
[crates-badge]: https://img.shields.io/crates/v/executor-core.svg
[crates-url]: https://crates.io/crates/executor-core

A flexible task executor abstraction layer for Rust async runtimes.

## Features

- Abstract interface for spawning async tasks
- Support for multiple executor backends:
  - [async-executor](https://crates.io/crates/async-executor) (default)
  - [tokio](https://crates.io/crates/tokio)
- Global and local executor variants
- Zero-cost abstractions
- Simple, lightweight API

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
executor-core = "0.1"
```

### Basic Example

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

### Feature Flags

- `default-async-executor` (default) - Use async-executor as the default implementation
- `default-tokio` - Use tokio as the default implementation
- `async-executor` - Enable async-executor support
- `tokio` - Enable tokio support

## Architecture

The crate provides these key traits:

- `Executor` - For spawning `Send` futures
- `LocalExecutor` - For spawning non-`Send` futures
- `Task` - Common interface for spawned tasks

And concrete implementations:

- Global executor accessible via `spawn()`
- Explicit executor instances when needed

## License

This project is licensed under the [MIT license](LICENSE).

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.