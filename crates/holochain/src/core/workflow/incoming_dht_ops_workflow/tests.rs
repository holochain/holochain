use super::*;
use crate::conductor::space::TestSpace;
use crate::core::workflow::error::WorkflowError;
use crate::core::{SysValidationError, ValidationOutcome};
use ::fixt::prelude::*;
use holo_hash::fixt::DnaHashFixturator;
use holo_hash::fixt::EntryHashFixturator;
use holochain_keystore::test_keystore;
use holochain_keystore::AgentPubKeyExt;
use holochain_state::dht_store::DhtStore;
use holochain_state::prelude::*;
use holochain_types::op::OpEntry;
use holochain_zome_types::fixt::{ActionFixturator, CreateAction, CreateLinkAction, UpdateAction};
use matches::assert_matches;

#[tokio::test(flavor = "multi_thread")]
async fn incoming_ops_to_limbo() {
    holochain_trace::test_run();

    let space = TestSpace::new(fixt!(DnaHash));
    let dht_store = space.space.dht_store.clone();
    let keystore = test_keystore();

    let author = fake_agent_pubkey_1();

    let mut hash_list = Vec::new();
    let mut op_list = Vec::new();

    for _ in 0..10 {
        let mut action = fixt!(Action, CreateLinkAction);
        action.header.author = author.clone();
        let action = action;
        let signature = author.sign(&keystore, &action).await.unwrap();

        let op = ChainOp::AgentActivity(SignedAction::new(action, signature));
        let hash = DhtOpHash::with_data_sync(&op);
        hash_list.push(hash);
        op_list.push(op);
    }

    let mut all = Vec::new();
    for op in op_list {
        let (sys_validation_trigger, _) = TriggerSender::new();
        let space = space.space.clone();
        all.push(tokio::task::spawn(async move {
            let start = std::time::Instant::now();
            incoming_dht_ops_workflow(space, sys_validation_trigger, vec![(op.into(), true)])
                .await
                .unwrap();
            println!("IN OP in {} s", start.elapsed().as_secs_f64());
        }));
    }

    futures::future::try_join_all(all).await.unwrap();

    verify_ops_present(&dht_store, hash_list, true).await;
}

// Checks that there is no other record of the op hash being held onto outside of the database that will prevent
// reprocessing.
#[tokio::test(flavor = "multi_thread")]
async fn can_retry_failed_op() {
    holochain_trace::test_run();

    let space = TestSpace::new(fixt!(DnaHash));
    let dht_store = space.space.dht_store.clone();
    let keystore = test_keystore();
    let (sys_validation_trigger, mut sys_validation_rx) = TriggerSender::new();

    let author = keystore.new_sign_keypair_random().await.unwrap();

    let mut action = fixt!(Action, CreateLinkAction);
    action.header.author = author.clone();
    let action = action;
    // Create a dummy signature that will fail validation
    let signature = Signature([0; SIGNATURE_BYTES]);

    let op: DhtOp = ChainOp::AgentActivity(SignedAction::new(action.clone(), signature)).into();
    let hash = DhtOpHash::with_data_sync(&op);

    // Try running the workflow and...
    let workflow_result = incoming_dht_ops_workflow(
        space.space.clone(),
        sys_validation_trigger.clone(),
        vec![(op, true)],
    )
    .await;

    // .. check that the workflow failed, with the ops NOT saved to the database
    assert!(workflow_result.is_err());
    verify_ops_present(&dht_store, vec![hash], false).await;

    // Now fix the signature
    let signature = author.sign(&keystore, &action).await.unwrap();
    let op: DhtOp = ChainOp::AgentActivity(SignedAction::new(action, signature)).into();
    let hash = DhtOpHash::with_data_sync(&op);

    // Run the workflow again to simulate a re-send of the op...
    incoming_dht_ops_workflow(
        space.space.clone(),
        sys_validation_trigger,
        vec![(op, true)],
    )
    .await
    .unwrap();

    // ... and now it should succeed
    verify_is_pending_validation_receipt(&dht_store, hash).await;
    // then sys validation should run on the new op
    sys_validation_rx.listen().await.unwrap();
    // and no ops should be claimed for processing
    assert!(space.space.incoming_op_hashes.0.lock().is_empty());
}

async fn verify_is_pending_validation_receipt(dht_store: &DhtStore, hash: DhtOpHash) {
    let found = dht_store.as_read().limbo_op_exists(&hash).await.unwrap();
    assert!(found, "op should be pending in limbo: {hash:?}");
}

/// A validation receipt is only requested for published ops.
/// Published ops have the flag, gossiped ops do not.
#[tokio::test(flavor = "multi_thread")]
async fn require_validation_receipt_follows_publish_flag() {
    holochain_trace::test_run();

    let space = TestSpace::new(fixt!(DnaHash));
    let dht_store = space.space.dht_store.clone();
    let keystore = test_keystore();
    let author = keystore.new_sign_keypair_random().await.unwrap();

    // Two distinct signed ops
    let mut ops = Vec::new();
    for _ in 0..2 {
        let mut action = fixt!(Action, CreateLinkAction);
        action.header.author = author.clone();
        let action = action;
        let signature = author.sign(&keystore, &action).await.unwrap();
        let op: DhtOp = ChainOp::AgentActivity(SignedAction::new(action, signature)).into();
        let hash = DhtOpHash::with_data_sync(&op);
        ops.push((op, hash));
    }
    let (published_op, published_hash) = ops[0].clone();
    let (gossiped_op, gossiped_hash) = ops[1].clone();

    let (sys_validation_trigger, _) = TriggerSender::new();
    incoming_dht_ops_workflow(
        space.space.clone(),
        sys_validation_trigger,
        // Published op requests a receipt, gossiped op does not.
        vec![(published_op, true), (gossiped_op, false)],
    )
    .await
    .unwrap();

    // Read the per-op receipt requirement from the DHT store's limbo: only the
    // published op should be flagged as requiring a validation receipt.
    let requiring = dht_store
        .as_read()
        .limbo_op_hashes_requiring_receipt()
        .await
        .unwrap();
    assert!(
        requiring.contains(&published_hash),
        "published op should require a validation receipt"
    );
    assert!(
        !requiring.contains(&gossiped_hash),
        "gossiped op should not require a validation receipt"
    );
}

async fn verify_ops_present(dht_store: &DhtStore, hash_list: Vec<DhtOpHash>, present: bool) {
    for hash in hash_list {
        let found = dht_store.as_read().limbo_op_exists(&hash).await.unwrap();
        assert_eq!(present, found);
    }
}

/// An op whose entry does not hash to the entry hash named by its action is
/// dropped at the gate: neither the op nor the fabricated entry reaches the
/// store (#5994). The entry is not covered by the action signature, so a
/// signed, genuine action can be paired with any entry bytes by a third party.
#[tokio::test(flavor = "multi_thread")]
async fn op_with_entry_not_matching_its_action_is_dropped() {
    holochain_trace::test_run();

    let space = TestSpace::new(fixt!(DnaHash));
    let dht_store = space.space.dht_store.clone();
    let keystore = test_keystore();
    let (sys_validation_trigger, _) = TriggerSender::new();
    let author = keystore.new_sign_keypair_random().await.unwrap();

    let entry = Entry::App(AppEntryBytes(SerializedBytes::from(UnsafeBytes::from(
        vec![1, 3, 5],
    ))));
    let real_hash = EntryHash::with_data_sync(&entry);
    let claimed_hash = fixt!(EntryHash);
    assert_ne!(real_hash, claimed_hash);

    // A genuinely signed create action that names a different entry hash than
    // the entry it is shipped with.
    let mut action = fixt!(Action, CreateAction);
    action.header.author = author.clone();
    *action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Public,
    ));
    *action.entry_hash_mut().unwrap() = claimed_hash.clone();
    let signature = author.sign(&keystore, &action).await.unwrap();
    let op: DhtOp = ChainOp::CreateEntry(
        SignedAction::new(action.clone(), signature),
        OpEntry::Present(entry.clone()),
    )
    .into();
    let hash = DhtOpHash::with_data_sync(&op);

    let result = incoming_dht_ops_workflow(
        space.space.clone(),
        sys_validation_trigger.clone(),
        vec![(op, true)],
    )
    .await;

    assert_matches!(
        result,
        Err(WorkflowError::SysValidationError(
            SysValidationError::ValidationOutcome(ValidationOutcome::EntryHash)
        ))
    );
    verify_ops_present(&dht_store, vec![hash], false).await;
    assert_eq!(
        dht_store
            .as_read()
            .retrieve_entry(&claimed_hash, None)
            .await
            .unwrap(),
        None,
        "the fabricated entry must not be stored under the claimed hash"
    );
    assert_eq!(
        dht_store
            .as_read()
            .retrieve_entry(&real_hash, None)
            .await
            .unwrap(),
        None,
        "the fabricated entry must not be stored under its own hash either"
    );

    // The same action naming the entry's real hash is accepted.
    *action.entry_hash_mut().unwrap() = real_hash;
    let signature = author.sign(&keystore, &action).await.unwrap();
    let op: DhtOp = ChainOp::CreateEntry(
        SignedAction::new(action, signature),
        OpEntry::Present(entry),
    )
    .into();
    let hash = DhtOpHash::with_data_sync(&op);
    incoming_dht_ops_workflow(
        space.space.clone(),
        sys_validation_trigger,
        vec![(op, true)],
    )
    .await
    .unwrap();
    verify_ops_present(&dht_store, vec![hash], true).await;
}

/// An op that ships an entry while its action has no entry hash cannot be
/// checked and is dropped at the gate, rather than failing later inside the
/// store write and taking the whole batch with it.
#[tokio::test(flavor = "multi_thread")]
async fn op_with_entry_but_no_entry_hash_on_action_is_dropped() {
    holochain_trace::test_run();

    let space = TestSpace::new(fixt!(DnaHash));
    let dht_store = space.space.dht_store.clone();
    let keystore = test_keystore();
    let (sys_validation_trigger, _) = TriggerSender::new();
    let author = keystore.new_sign_keypair_random().await.unwrap();

    let mut action = fixt!(Action, CreateLinkAction);
    action.header.author = author.clone();
    let action = action;
    assert!(action.entry_hash().is_none());
    let signature = author.sign(&keystore, &action).await.unwrap();
    let op: DhtOp = ChainOp::CreateRecord(
        SignedAction::new(action, signature),
        OpEntry::Present(fixt!(Entry)),
    )
    .into();
    let hash = DhtOpHash::with_data_sync(&op);

    let result = incoming_dht_ops_workflow(
        space.space.clone(),
        sys_validation_trigger,
        vec![(op, true)],
    )
    .await;

    assert_matches!(
        result,
        Err(WorkflowError::SysValidationError(
            SysValidationError::ValidationOutcome(ValidationOutcome::MalformedDhtOp(..))
        ))
    );
    verify_ops_present(&dht_store, vec![hash], false).await;
}

/// An update op ships the new entry, which must hash to the entry hash named
/// by the update action, just like a create.
#[tokio::test(flavor = "multi_thread")]
async fn update_op_with_entry_not_matching_its_action_is_dropped() {
    holochain_trace::test_run();

    let space = TestSpace::new(fixt!(DnaHash));
    let dht_store = space.space.dht_store.clone();
    let keystore = test_keystore();
    let (sys_validation_trigger, _) = TriggerSender::new();
    let author = keystore.new_sign_keypair_random().await.unwrap();

    let entry = Entry::App(AppEntryBytes(SerializedBytes::from(UnsafeBytes::from(
        vec![1, 3, 5],
    ))));
    let real_hash = EntryHash::with_data_sync(&entry);

    let mut action = fixt!(Action, UpdateAction);
    action.header.author = author.clone();
    *action.entry_hash_mut().unwrap() = fixt!(EntryHash);
    let signature = author.sign(&keystore, &action).await.unwrap();
    let op: DhtOp = ChainOp::UpdateRecord(
        SignedAction::new(action.clone(), signature),
        OpEntry::Present(entry.clone()),
    )
    .into();
    let hash = DhtOpHash::with_data_sync(&op);

    let result = incoming_dht_ops_workflow(
        space.space.clone(),
        sys_validation_trigger.clone(),
        vec![(op, true)],
    )
    .await;

    assert_matches!(
        result,
        Err(WorkflowError::SysValidationError(
            SysValidationError::ValidationOutcome(ValidationOutcome::EntryHash)
        ))
    );
    verify_ops_present(&dht_store, vec![hash], false).await;
    assert_eq!(
        dht_store
            .as_read()
            .retrieve_entry(&real_hash, None)
            .await
            .unwrap(),
        None,
    );

    *action.entry_hash_mut().unwrap() = real_hash;
    let signature = author.sign(&keystore, &action).await.unwrap();
    let op: DhtOp = ChainOp::UpdateRecord(
        SignedAction::new(action, signature),
        OpEntry::Present(entry),
    )
    .into();
    let hash = DhtOpHash::with_data_sync(&op);
    incoming_dht_ops_workflow(
        space.space.clone(),
        sys_validation_trigger,
        vec![(op, true)],
    )
    .await
    .unwrap();
    verify_ops_present(&dht_store, vec![hash], true).await;
}

/// An op with a mismatching entry is dropped on its own: valid ops delivered
/// in the same batch are still stored.
#[tokio::test(flavor = "multi_thread")]
async fn op_with_bad_entry_does_not_drop_valid_ops_in_batch() {
    holochain_trace::test_run();

    let space = TestSpace::new(fixt!(DnaHash));
    let dht_store = space.space.dht_store.clone();
    let keystore = test_keystore();
    let (sys_validation_trigger, _) = TriggerSender::new();
    let author = keystore.new_sign_keypair_random().await.unwrap();

    let entry = Entry::App(AppEntryBytes(SerializedBytes::from(UnsafeBytes::from(
        vec![1, 3, 5],
    ))));

    let mut bad_action = fixt!(Action, CreateAction);
    bad_action.header.author = author.clone();
    *bad_action.entry_hash_mut().unwrap() = fixt!(EntryHash);
    let signature = author.sign(&keystore, &bad_action).await.unwrap();
    let bad_op: DhtOp = ChainOp::CreateEntry(
        SignedAction::new(bad_action, signature),
        OpEntry::Present(entry.clone()),
    )
    .into();
    let bad_hash = DhtOpHash::with_data_sync(&bad_op);

    let mut good_action = fixt!(Action, CreateLinkAction);
    good_action.header.author = author.clone();
    let signature = author.sign(&keystore, &good_action).await.unwrap();
    let good_op: DhtOp = ChainOp::CreateLink(SignedAction::new(good_action, signature)).into();
    let good_hash = DhtOpHash::with_data_sync(&good_op);

    incoming_dht_ops_workflow(
        space.space.clone(),
        sys_validation_trigger,
        vec![(bad_op, true), (good_op, true)],
    )
    .await
    .unwrap();

    verify_ops_present(&dht_store, vec![good_hash], true).await;
    verify_ops_present(&dht_store, vec![bad_hash], false).await;
}
