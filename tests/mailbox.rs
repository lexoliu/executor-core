#![cfg(all(feature = "std", feature = "tokio"))]

use executor_core::mailbox::Mailbox;
use executor_core::tokio::{LocalSet, Runtime};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

#[test]
fn mailbox_handles_updates_and_calls() {
    let runtime = Runtime::new().expect("runtime");
    let handle = runtime.handle().clone();
    let local_set = LocalSet::new();

    let (sum, len) = handle.block_on(local_set.run_until(async {
        let mailbox = Mailbox::new(&local_set, RefCell::new(HashMap::<String, usize>::new()));

        mailbox.handle(|map| {
            map.borrow_mut().insert("a".to_string(), 1);
        });
        mailbox.handle(|map| {
            map.borrow_mut().insert("b".to_string(), 2);
        });

        let len = mailbox.call(|map| map.borrow().len()).await;
        let sum = mailbox
            .call(|map| map.borrow().values().copied().sum::<usize>())
            .await;

        (sum, len)
    }));

    assert_eq!(sum, 3);
    assert_eq!(len, 2);
}

#[test]
fn mailbox_supports_non_send_state() {
    let runtime = Runtime::new().expect("runtime");
    let handle = runtime.handle().clone();
    let local_set = LocalSet::new();

    let value: usize = handle.block_on(local_set.run_until(async {
        let mailbox = Mailbox::new(&local_set, RefCell::new(Vec::<Rc<usize>>::with_capacity(1)));

        mailbox.handle(|vec| {
            vec.borrow_mut().push(Rc::new(7));
        });

        mailbox.call(|vec| *vec.borrow()[0]).await
    }));

    assert_eq!(value, 7);
}
