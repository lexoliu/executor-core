#![cfg(all(feature = "std", feature = "tokio"))]

use executor_core::{AnyExecutor, AnyLocalExecutor, Executor, LocalExecutor};
use executor_core::tokio::{LocalSet, Runtime};

#[test]
fn any_executor_spawn_and_downcast() {
    let runtime = Runtime::new().expect("runtime");
    let handle = runtime.handle().clone();
    let any = AnyExecutor::new(runtime);

    assert!(any.downcast_ref::<Runtime>().is_some());

    let task = any.spawn(async { 7usize });
    let value = handle.block_on(task);
    assert_eq!(value, 7);

    let runtime = any.downcast::<Runtime>().expect("downcast runtime");
    let second = runtime.block_on(runtime.spawn(async { 11usize }));
    assert_eq!(second, 11);
}

#[test]
fn any_local_executor_spawn_and_downcast() {
    let runtime = Runtime::new().expect("runtime");
    let handle = runtime.handle().clone();
    let any_local = AnyLocalExecutor::new(LocalSet::new());

    let result = handle.block_on({
        let any_local = any_local;
        async move {
            let local_ref = any_local
                .downcast_ref::<LocalSet>()
                .expect("local set present");

            local_ref
                .run_until(async {
                    let task = any_local.spawn_local(async { 21usize });
                    task.await
                })
                .await
        }
    });

    assert_eq!(result, 21);

    let recovered = AnyLocalExecutor::new(LocalSet::new())
        .downcast::<LocalSet>()
        .expect("downcast local set");
    let second = handle.block_on(recovered.run_until(async { 8usize }));
    assert_eq!(second, 8);
}
