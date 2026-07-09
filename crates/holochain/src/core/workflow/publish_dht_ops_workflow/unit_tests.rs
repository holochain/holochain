use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use crate::core::workflow::publish_dht_ops_workflow::publish_dht_ops_workflow;
use crate::prelude::*;
use ::fixt::prelude::*;
use holo_hash::fixt::ActionHashFixturator;
use holo_hash::fixt::AgentPubKeyFixturator;
use holo_hash::fixt::DnaHashFixturator;
use holo_hash::fixt::EntryHashFixturator;
use holo_hash::AgentPubKey;
use holo_hash::DhtOpHash;
use holo_hash::HasHash;
use holochain_conductor_api::conductor::ConductorTuningParams;
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_state::dht_store::DhtStore;
use holochain_state::prelude::*;
use holochain_state::test_utils::test_dht_store;
use holochain_types::dht_v2::{ChainOp, DhtOp, DhtOpHashed, OpEntry, SignedAction};
use holochain_zome_types::dependencies::holochain_integrity_types::action::Action;
use holochain_zome_types::dht_v2::from_legacy_action;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
async fn no_ops_to_publish() {
    holochain_trace::test_run();

    let dht_store = test_dht_store(fixt!(DnaHash)).await;

    let mut network = MockHolochainP2pDnaT::new();
    network.expect_publish().never();

    let (tx, rx) =
        TriggerSender::new_with_loop(Duration::from_secs(5)..Duration::from_secs(30), true);

    let work_complete = publish_dht_ops_workflow(
        dht_store,
        Arc::new(network),
        tx,
        fixt!(AgentPubKey),
        ConductorTuningParams::default().min_publish_interval(),
    )
    .await
    .unwrap();

    assert_eq!(WorkComplete::Complete, work_complete);
    assert!(rx.is_paused());
}

#[tokio::test(flavor = "multi_thread")]
async fn workflow_incomplete_on_routing_error() {
    holochain_trace::test_run();

    let dht_store = test_dht_store(fixt!(DnaHash)).await;

    let agent = fixt!(AgentPubKey);

    let op_hash = create_op(&dht_store, agent.clone()).await.unwrap();

    let mut network = MockHolochainP2pDnaT::new();
    network.expect_publish().return_once(|_, _, _, _| {
        Err(holochain_p2p::HolochainP2pError::RoutingDnaError(fixt!(
            DnaHash
        )))
    });

    let (tx, rx) =
        TriggerSender::new_with_loop(Duration::from_secs(5)..Duration::from_secs(30), true);

    let work_complete = publish_dht_ops_workflow(
        dht_store.clone(),
        Arc::new(network),
        tx,
        agent,
        ConductorTuningParams::default().min_publish_interval(),
    )
    .await
    .unwrap();

    let publish_timestamp = get_publish_time(&dht_store, op_hash).await;

    assert_eq!(WorkComplete::Incomplete(None), work_complete);
    assert!(!rx.is_paused());
    assert!(publish_timestamp.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn workflow_handles_publish_errors() {
    holochain_trace::test_run();

    let dht_store = test_dht_store(fixt!(DnaHash)).await;

    let agent = fixt!(AgentPubKey);

    let op_hash = create_op(&dht_store, agent.clone()).await.unwrap();

    let mut network = MockHolochainP2pDnaT::new();
    network.expect_publish().return_once(|_, _, _, _| {
        Err(holochain_p2p::HolochainP2pError::InvalidP2pMessage(
            "test error".to_string(),
        ))
    });

    let (tx, rx) =
        TriggerSender::new_with_loop(Duration::from_secs(5)..Duration::from_secs(30), true);

    let work_complete = publish_dht_ops_workflow(
        dht_store.clone(),
        Arc::new(network),
        tx,
        agent,
        ConductorTuningParams::default().min_publish_interval(),
    )
    .await
    .unwrap();

    let publish_timestamp = get_publish_time(&dht_store, op_hash).await;

    assert_eq!(WorkComplete::Complete, work_complete);
    assert!(!rx.is_paused());
    assert!(publish_timestamp.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn retry_publish_until_receipts_received() {
    holochain_trace::test_run();

    let dht_store = test_dht_store(fixt!(DnaHash)).await;

    let agent = fixt!(AgentPubKey);

    let op_hash = create_op(&dht_store, agent.clone()).await.unwrap();

    let mut network = MockHolochainP2pDnaT::new();
    network.expect_publish().returning(|_, _, _, _| Ok(()));

    let (tx, rx) =
        TriggerSender::new_with_loop(Duration::from_secs(5)..Duration::from_secs(30), true);

    let network = Arc::new(network);

    for _ in 0..3 {
        let work_complete = publish_dht_ops_workflow(
            dht_store.clone(),
            network.clone(),
            tx.clone(),
            agent.clone(),
            ConductorTuningParams::default().min_publish_interval(),
        )
        .await
        .unwrap();

        // The work should complete but the trigger shouldn't pause so that the workflow keeps publishing until
        // enough validation receipts have been received for this op
        assert_eq!(WorkComplete::Complete, work_complete);
        assert!(!rx.is_paused());

        verify_published_recently(&dht_store, op_hash.clone()).await;
    }

    do_set_receipts_complete(&dht_store, op_hash.clone()).await;

    let work_complete = publish_dht_ops_workflow(
        dht_store,
        network,
        tx,
        agent,
        ConductorTuningParams::default().min_publish_interval(),
    )
    .await
    .unwrap();

    assert_eq!(WorkComplete::Complete, work_complete);
    assert!(rx.is_paused()); // Should now pause, no more work to do
}

#[tokio::test(flavor = "multi_thread")]
async fn loop_resumes_on_new_data() {
    holochain_trace::test_run();

    let dht_store = test_dht_store(fixt!(DnaHash)).await;

    let agent = fixt!(AgentPubKey);

    let mut network = MockHolochainP2pDnaT::new();
    network.expect_publish().returning(|_, _, _, _| Ok(()));

    let (tx, rx) =
        TriggerSender::new_with_loop(Duration::from_secs(5)..Duration::from_secs(30), true);

    let network = Arc::new(network);

    // Do a publish with no data to get into a paused state
    let work_complete = publish_dht_ops_workflow(
        dht_store.clone(),
        network.clone(),
        tx.clone(),
        agent.clone(),
        ConductorTuningParams::default().min_publish_interval(),
    )
    .await
    .unwrap();

    assert_eq!(WorkComplete::Complete, work_complete);
    assert!(rx.is_paused()); // No work to do, so it should pause

    // Now create an op and try to publish again
    create_op(&dht_store, agent.clone()).await.unwrap();

    let work_complete = publish_dht_ops_workflow(
        dht_store,
        network,
        tx,
        agent.clone(),
        ConductorTuningParams::default().min_publish_interval(),
    )
    .await
    .unwrap();

    assert_eq!(WorkComplete::Complete, work_complete);
    assert!(!rx.is_paused()); // No validation receipts yet so might need to publish again, should it should resume
}

#[tokio::test(flavor = "multi_thread")]
async fn ignores_data_by_other_authors() {
    holochain_trace::test_run();

    let dht_store = test_dht_store(fixt!(DnaHash)).await;

    // Create an op for some other author
    create_op(&dht_store, fixt!(AgentPubKey)).await.unwrap();

    let agent = fixt!(AgentPubKey);

    let mut network = MockHolochainP2pDnaT::new();
    network.expect_publish().never();

    let (tx, rx) =
        TriggerSender::new_with_loop(Duration::from_secs(5)..Duration::from_secs(30), true);

    let network = Arc::new(network);

    let work_complete = publish_dht_ops_workflow(
        dht_store,
        network.clone(),
        tx.clone(),
        agent.clone(),
        ConductorTuningParams::default().min_publish_interval(),
    )
    .await
    .unwrap();

    // Should be nothing to do, so complete and paused
    assert_eq!(WorkComplete::Complete, work_complete);
    assert!(rx.is_paused());
}

// Even though ops are created for actions with private entries, the StoreEntry
// op (which carries the entry) must not be published.
#[tokio::test(flavor = "multi_thread")]
async fn private_entries_are_not_published() {
    holochain_trace::test_run();

    let dht_store = test_dht_store(fixt!(DnaHash)).await;
    let agent = fixt!(AgentPubKey);

    // Create a private entry.
    let create_action = Create {
        action_seq: 5,
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: Default::default(),
        author: agent.clone(),
        entry_hash: fixt!(EntryHash),
        entry_type: EntryType::App(AppEntryDef {
            entry_index: 0.into(),
            zome_index: 0.into(),
            visibility: EntryVisibility::Private,
        }),
    };
    let action = Action::Create(create_action.clone());
    let v2_action = from_legacy_action(&action);

    let register_agent_activity_op = DhtOpHashed::from_content_sync(DhtOp::from(
        ChainOp::AgentActivity(SignedAction::new(v2_action.clone(), fixt!(Signature))),
    ));
    let store_entry_op = DhtOpHashed::from_content_sync(DhtOp::from(ChainOp::CreateEntry(
        SignedAction::new(v2_action.clone(), fixt!(Signature)),
        OpEntry::Present(fixt!(Entry)),
    )));
    let store_record_op = DhtOpHashed::from_content_sync(DhtOp::from(ChainOp::CreateRecord(
        SignedAction::new(v2_action, fixt!(Signature)),
        OpEntry::Hidden,
    )));

    // The v2 AgentActivity op structurally carries no entry, so the private
    // entry cannot leak through the op that gets published.
    match register_agent_activity_op.as_content() {
        DhtOp::ChainOp(op) => assert!(op.op_entry().is_none()),
        DhtOp::WarrantOp(_) => panic!("expected a chain op"),
    }

    let register_agent_activity_op_hash = register_agent_activity_op.as_hash().clone();
    let store_entry_op_hash = store_entry_op.as_hash().clone();
    let store_record_op_hash = store_record_op.as_hash().clone();

    // Seed all three ops as integrated, self-authored ops in the DHT store.
    for op in [register_agent_activity_op, store_entry_op, store_record_op] {
        dht_store
            .test_insert_authored_chain_op(op, None, None, None)
            .await
            .unwrap();
    }

    // RegisterAgentActivity and StoreRecord are expected to be published.
    // StoreEntry contains the entry and is expected to not be published.
    let mut network = MockHolochainP2pDnaT::new();
    let agent2 = agent.clone();
    network
        .expect_publish()
        .returning(move |_basis_hash, source, op_hash_list, _timeout_ms| {
            assert_eq!(source, agent2);
            assert!(
                op_hash_list.contains(&register_agent_activity_op_hash)
                    || op_hash_list.contains(&store_record_op_hash)
            );
            assert!(!op_hash_list.contains(&store_entry_op_hash));
            Ok(())
        });
    let network = Arc::new(network);

    let (tx, _rx) =
        TriggerSender::new_with_loop(Duration::from_secs(5)..Duration::from_secs(30), true);

    let work_complete = publish_dht_ops_workflow(
        dht_store,
        network.clone(),
        tx.clone(),
        agent.clone(),
        ConductorTuningParams::default().min_publish_interval(),
    )
    .await
    .unwrap();

    // Complete just means there have not been errors during publish.
    assert_eq!(WorkComplete::Complete, work_complete);
}

async fn verify_published_recently(dht_store: &DhtStore, op_hash: DhtOpHash) {
    let publish_timestamp = get_publish_time(dht_store, op_hash)
        .await
        .expect("Expected published time to have been set");

    // Published within the last second.
    assert!(
        publish_timestamp.as_micros() + 1_000_000 > Timestamp::now().as_micros(),
        "publish time {publish_timestamp:?} is not recent"
    );
}

/// Seed an integrated, self-authored `RegisterAgentActivity` op (with an empty
/// publish row) and return its hash.
async fn create_op(dht_store: &DhtStore, author: AgentPubKey) -> StateMutationResult<DhtOpHash> {
    let mut create_action = fixt!(Create);
    create_action.author = author;
    let action = Action::Create(create_action);

    let signed = SignedAction::new(from_legacy_action(&action), fixt!(Signature));
    let op = DhtOpHashed::from_content_sync(DhtOp::from(ChainOp::AgentActivity(signed)));

    let op_hash = op.as_hash().clone();
    dht_store
        .test_insert_authored_chain_op(op, None, None, None)
        .await?;

    Ok(op_hash)
}

async fn get_publish_time(dht_store: &DhtStore, op_hash: DhtOpHash) -> Option<Timestamp> {
    dht_store
        .test_chain_op_publish_time(&op_hash)
        .await
        .unwrap()
}

async fn do_set_receipts_complete(dht_store: &DhtStore, op_hash: DhtOpHash) {
    dht_store
        .mark_chain_op_receipts_complete(&op_hash)
        .await
        .unwrap();
}

/// Build an `InvalidChainOp` warrant op authored by `agent`.
fn build_warrant_op(agent: &AgentPubKey) -> DhtOpHashed {
    let warrant = SignedWarrant::new(
        Warrant::new(
            WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
                action_author: fixt!(AgentPubKey),
                action: (fixt!(ActionHash), fixt!(Signature)),
                chain_op_type: ChainOpType::RegisterAddLink,
                reason: "test warrant".into(),
            }),
            agent.clone(),
            Timestamp::now(),
            fixt!(AgentPubKey),
        ),
        fixt!(Signature),
    );
    DhtOpHashed::from_content_sync(DhtOp::from(warrant))
}

/// The workflow publishes an integrated, self-authored warrant and records that
/// it was published, so it is not published again. Unlike chain ops, a warrant
/// publishes once — it needs no validation receipts.
#[tokio::test(flavor = "multi_thread")]
async fn workflow_publishes_warrant_once() {
    holochain_trace::test_run();

    let dht_store = test_dht_store(fixt!(DnaHash)).await;
    let agent = fixt!(AgentPubKey);

    let warrant_op = build_warrant_op(&agent);
    dht_store
        .test_insert_integrated_warrant(warrant_op)
        .await
        .unwrap();

    // First run: the warrant is eligible, so the workflow publishes it once.
    let mut network = MockHolochainP2pDnaT::new();
    network
        .expect_publish()
        .times(1)
        .returning(|_, _, _, _| Ok(()));
    let (tx, _rx) =
        TriggerSender::new_with_loop(Duration::from_secs(5)..Duration::from_secs(30), true);
    publish_dht_ops_workflow(
        dht_store.clone(),
        Arc::new(network),
        tx,
        agent.clone(),
        ConductorTuningParams::default().min_publish_interval(),
    )
    .await
    .unwrap();

    // Second run: the workflow recorded the publish, so the warrant is no longer
    // eligible and must not be published again.
    let mut network = MockHolochainP2pDnaT::new();
    network.expect_publish().never();
    let (tx, _rx) =
        TriggerSender::new_with_loop(Duration::from_secs(5)..Duration::from_secs(30), true);
    publish_dht_ops_workflow(
        dht_store,
        Arc::new(network),
        tx,
        agent,
        ConductorTuningParams::default().min_publish_interval(),
    )
    .await
    .unwrap();
}
