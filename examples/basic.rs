//! Basic example demonstrating how to use executor-core for task spawning and awaiting.

use executor_core::spawn;

#[tokio::main]
async fn main() {
    // Spawn a simple task on the global executor
    let task = spawn(async {
        println!("Hello from a spawned task!");
        // Simulate some work
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        42
    });

    // Await the task result
    let result = task.await;
    println!("Task completed with result: {result}");

    // Spawn multiple tasks and collect their results
    let tasks: Vec<_> = (1..=5)
        .map(|i| {
            spawn(async move {
                println!("Task {i} starting");
                tokio::time::sleep(std::time::Duration::from_millis(50 * i)).await;
                println!("Task {i} completed");
                i * 10
            })
        })
        .collect();

    // Gather all results in order
    for (i, task) in tasks.into_iter().enumerate() {
        let result = task.await;
        println!("Task {} result: {}", i + 1, result);
    }

    println!("All tasks completed!");
}
