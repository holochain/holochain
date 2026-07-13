use super::*;
use crate::core::queue_consumer::TriggerSender;
use ::fixt::prelude::*;
use holo_hash::fixt::{AgentPubKeyFixturator, DnaHashFixturator};
use holo_hash::AgentPubKey;
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_state::dht_store::DhtStore;
use holochain_state::prelude::SysOutcome;
use holochain_state::test_utils::test_dht_store;
use holochain_types::dht_v2::{ChainOp, DhtOp, DhtOpHashed, OpEntry, SignedAction};
use holochain_zome_types::dependencies::holochain_integrity_types::dht_v2::{
    Action, ActionData, ActionHeader, CreateData,
};
use kitsune2_api::StoredOp;
use std::sync::Arc;

/// Build an [`Action`] from a fixturated `Create` struct.
fn v2_create(c: Create) -> Action {
    Action {
        header: ActionHeader {
            author: c.author,
            timestamp: c.timestamp,
            action_seq: c.action_seq,
            prev_action: Some(c.prev_action),
        },
        data: ActionData::Create(CreateData {
            entry_type: c.entry_type,
            entry_hash: c.entry_hash,
        }),
    }
}

// TESTS BEGIN HERE

#[tokio::test(flavor = "multi_thread")]
async fn inform_kitsune_about_integrated_ops() {
    let tests = [
        make_store_entry_op_pair as fn() -> (DhtOp, DhtOpHashed),
        make_store_record_op_pair,
    ];
    for (i, make_op) in tests.iter().enumerate() {
        let dna_hash = fixt!(DnaHash);
        let (op, hashed) = make_op();
        let dht_store = test_dht_store(dna_hash.clone()).await;
        insert_validated_op_to_store(&dht_store, &hashed).await;

        let (tx, _rx) = TriggerSender::new();
        let mut hc_p2p = MockHolochainP2pDnaT::new();
        hc_p2p.expect_dna_hash().return_const(dna_hash.clone());
        hc_p2p
            .expect_new_integrated_data()
            .times(1)
            .return_once(move |ops| {
                let authored_at = match &op {
                    DhtOp::ChainOp(c) => c.signed_action().data().timestamp(),
                    DhtOp::WarrantOp(w) => w.timestamp,
                };
                let expected_op = StoredOp {
                    op_id: op.to_hash().to_located_k2_op_id(&op.dht_basis()),
                    created_at: kitsune2_api::Timestamp::from_micros(authored_at.as_micros()),
                };
                assert_eq!(ops, vec![expected_op], "test case {i}");
                Ok(())
            });
        let hc_p2p = Arc::new(hc_p2p);
        integrate_dht_ops_workflow(dht_store, tx, hc_p2p)
            .await
            .unwrap();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn kitsune_not_informed_when_no_ops_integrated() {
    let dna_hash = fixt!(DnaHash);
    // An empty store — nothing ready for integration.
    let dht_store = test_dht_store(dna_hash.clone()).await;

    let (tx, _rx) = TriggerSender::new();
    let mut hc_p2p = MockHolochainP2pDnaT::new();
    hc_p2p.expect_dna_hash().return_const(dna_hash.clone());
    hc_p2p.expect_new_integrated_data().never();
    let hc_p2p = Arc::new(hc_p2p);
    integrate_dht_ops_workflow(dht_store, tx, hc_p2p)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn single_local_author_marked_integrated() {
    holochain_trace::test_run();
    let dna_hash = fixt!(DnaHash);
    let author = fixt!(AgentPubKey);
    let (_op, hashed) = make_store_entry_op(author.clone());

    let dht_store = test_dht_store(dna_hash.clone()).await;
    insert_validated_op_to_store(&dht_store, &hashed).await;

    let (tx, _rx) = TriggerSender::new();
    let mut hc_p2p = MockHolochainP2pDnaT::new();
    hc_p2p.expect_dna_hash().return_const(dna_hash.clone());
    hc_p2p.expect_new_integrated_data().return_once(move |ops| {
        assert_eq!(ops.len(), 1);
        Ok(())
    });
    let mock_network = Arc::new(hc_p2p);

    integrate_dht_ops_workflow(dht_store.clone(), tx, mock_network)
        .await
        .unwrap();

    let hash = hashed.as_hash().clone();
    assert!(
        dht_store.when_integrated(&hash).await.unwrap().is_some(),
        "Op should be marked as integrated in DHT store"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn multiple_local_authors_marked_integrated() {
    holochain_trace::test_run();
    let dna_hash = fixt!(DnaHash);
    let author_a = fixt!(AgentPubKey);
    let author_b = fixt!(AgentPubKey);
    let (_op_a, hashed_a) = make_store_entry_op(author_a.clone());
    let (_op_b, hashed_b) = make_store_entry_op(author_b.clone());

    let dht_store = test_dht_store(dna_hash.clone()).await;
    insert_validated_op_to_store(&dht_store, &hashed_a).await;
    insert_validated_op_to_store(&dht_store, &hashed_b).await;

    let (tx, _rx) = TriggerSender::new();
    let mut hc_p2p = MockHolochainP2pDnaT::new();
    hc_p2p.expect_dna_hash().return_const(dna_hash.clone());
    hc_p2p
        .expect_new_integrated_data()
        .return_once(move |mut ops| {
            ops.sort_by(|a, b| a.op_id.cmp(&b.op_id));
            assert_eq!(ops.len(), 2);
            Ok(())
        });
    let mock_network = Arc::new(hc_p2p);

    integrate_dht_ops_workflow(dht_store.clone(), tx, mock_network)
        .await
        .unwrap();

    let hash_a = hashed_a.as_hash().clone();
    let hash_b = hashed_b.as_hash().clone();
    assert!(dht_store.when_integrated(&hash_a).await.unwrap().is_some());
    assert!(dht_store.when_integrated(&hash_b).await.unwrap().is_some());
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Write an op to the new DhtStore as fully validated and ready for
/// integration (sys + app validation both accepted).
async fn insert_validated_op_to_store(dht_store: &DhtStore, op: &DhtOpHashed) {
    let op_hash = op.as_hash().clone();
    dht_store
        .record_incoming_ops(vec![(op.clone(), false)])
        .await
        .unwrap();
    dht_store
        .record_chain_op_sys_validation_outcomes(vec![(op_hash.clone(), SysOutcome::Accepted)])
        .await
        .unwrap();
    dht_store
        .record_app_validation_outcomes(vec![(
            op_hash,
            holochain_state::prelude::AppOutcome::Accepted,
        )])
        .await
        .unwrap();
}

fn make_store_entry_op(author: AgentPubKey) -> (DhtOp, DhtOpHashed) {
    let entry = EntryFixturator::new(AppEntry).next().unwrap();
    let mut action = fixt!(Create);
    action.author = author;
    action.entry_hash = EntryHashed::from_content_sync(entry.clone()).into_hash();
    let v2_action = v2_create(action);
    let signed = SignedAction::new(v2_action, fixt!(Signature));
    let op: DhtOp = ChainOp::CreateEntry(signed, OpEntry::Present(entry)).into();
    let hashed = DhtOpHashed::from_content_sync(op.clone());
    (op, hashed)
}

fn make_store_entry_op_pair() -> (DhtOp, DhtOpHashed) {
    make_store_entry_op(fixt!(AgentPubKey))
}

fn make_store_record_op_pair() -> (DhtOp, DhtOpHashed) {
    let entry = EntryFixturator::new(AppEntry).next().unwrap();
    let mut action = fixt!(Create);
    action.author = fixt!(AgentPubKey);
    action.entry_hash = EntryHashed::from_content_sync(entry.clone()).into_hash();
    let v2_action = v2_create(action);
    let signed = SignedAction::new(v2_action, fixt!(Signature));
    let op: DhtOp = ChainOp::CreateRecord(signed, OpEntry::Present(entry)).into();
    let hashed = DhtOpHashed::from_content_sync(op.clone());
    (op, hashed)
}
