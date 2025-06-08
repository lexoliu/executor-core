//! Error handling example demonstrating how to deal with task panics and cancellation.

use executor_core::{Error, Task, spawn};
use std::time::Duration;

#[tokio::main]
async fn main() {
    // Example 1: Handling task panics
    let panicking_task = spawn(async {
        println!("This task will panic!");
        tokio::time::sleep(Duration::from_millis(100)).await;
        panic!("Intentional panic for demonstration");
        #[allow(unreachable_code)]
        42 // This will never be reached
    });

    // Using result() to handle the panic gracefully
    match panicking_task.result().await {
        Ok(value) => println!("Task completed successfully with: {value}"),
        Err(Error::Panicked(msg)) => println!("Task panicked as expected: {msg}"),
        Err(Error::Cancelled) => println!("Task was cancelled"),
    }

    // Example 2: Cancelling a running task
    let task_to_cancel = spawn(async {
        println!("Starting a long-running task...");
        for i in 1..=10 {
            println!("Working... step {i}/10");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        println!("Task completed!");
        "Success"
    });

    // Let it run for a bit
    tokio::time::sleep(Duration::from_millis(250)).await;

    // Then cancel it
    println!("Cancelling the task before completion");
    task_to_cancel.cancel();
    println!("Task has been cancelled");

    // Example 3: Using a timeout pattern
    let slow_task = spawn(async {
        println!("Starting slow computation...");
        tokio::time::sleep(Duration::from_secs(2)).await;
        println!("Slow computation finished!");
        100
    });

    match tokio::time::timeout(Duration::from_millis(500), slow_task).await {
        Ok(result) => println!("Task completed within timeout: {result}"),
        Err(_) => println!("Task did not complete within timeout"),
    }

    // Note: The task continues running in the background after timeout
    tokio::time::sleep(Duration::from_secs(2)).await;
    println!("Done with all examples");
}
