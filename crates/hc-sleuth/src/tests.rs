use arbitrary::Arbitrary;
use holochain_state::mutations;

use crate::{query::*, *};

#[test]
fn all_integrated() {
    let mut u = unstructured_noise();
    let alice = NodeEnv::test();
    let bobbo = NodeEnv::test();
    let carol = NodeEnv::test();

    let op = DhtOpHashed::arbitrary(&mut u).unwrap();
    let op1 = op.clone();
    let op2 = op.clone();

    bobbo.authored.test_write(move |txn| {
        mutations::insert_op(txn, &op1).unwrap();
    });
    carol.dht.test_write(move |txn| {
        mutations::insert_op(txn, &op2).unwrap();
        mutations::set_when_integrated(txn, &op2.hash, Timestamp::now()).unwrap();
    });

    // let report = action_report(&alice, &[bobbo, carol], op.hash, ItemStatus::Integrated).unwrap();

    // assert_eq!(report, ActionReport::Fail { step: None });
}
