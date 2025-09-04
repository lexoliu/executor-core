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

Write async libraries without choosing a runtime.

Your users should decide whether to use tokio, async-std, or any other runtime. Not you.

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

**Library authors:**

```toml
[dependencies]
executor-core = "0.2"
```

**App developers:**

```toml
[dependencies]
executor-core = { version = "0.2", features = ["tokio"] }
```

## API

Two traits:

- `Executor` - For `Send` futures
- `LocalExecutor` - For non-`Send` futures

Both return `async_task::Task`:

```rust
let task = executor.spawn(async { work() });
let result = task.await;    // Get result
task.cancel().await;         // Cancel task
task.detach();              // Run in background
```

## Supported Runtimes

| Runtime        | Feature            |
| -------------- | ------------------ |
| tokio          | `"tokio"`          |
| async-executor | `"async-executor"` |
| Web/WASM       | `"web"`            |

## License

MIT
