use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use crate::core::workflow::publish_dht_ops_workflow::publish_dht_ops_workflow;
use crate::prelude::*;
use ::fixt::prelude::*;
use chrono::Utc;
use hdk::prelude::Action;
use holo_hash::fixt::ActionHashFixturator;
use holo_hash::fixt::AgentPubKeyFixturator;
use holo_hash::fixt::DnaHashFixturator;
use holo_hash::fixt::EntryHashFixturator;
use holo_hash::AgentPubKey;
use holo_hash::HasHash;
use holochain_conductor_api::conductor::ConductorTuningParams;
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_sqlite::db::DbKindAuthored;
use holochain_sqlite::prelude::*;
use holochain_state::prelude::*;
use rusqlite::named_params;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

#[tokio::test(flavor = "multi_thread")]
async fn no_ops_to_publish() {
    holochain_trace::test_run();

    let test_db = holochain_state::test_utils::test_authored_db();
    let vault = test_db.to_db();

    let mut network = MockHolochainP2pDnaT::new();
    network.expect_publish().never();

    let (tx, rx) =
        TriggerSender::new_with_loop(Duration::from_secs(5)..Duration::from_secs(30), true);

    let work_complete = publish_dht_ops_workflow(
        vault,
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

    let test_db = holochain_state::test_utils::test_authored_db();
    let vault = test_db.to_db();

    let agent = fixt!(AgentPubKey);

    let op_hash = create_op(vault.clone(), agent.clone()).await.unwrap();

    let mut network = MockHolochainP2pDnaT::new();
    network.expect_publish().return_once(|_, _, _, _, _, _| {
        Err(holochain_p2p::HolochainP2pError::RoutingDnaError(fixt!(
            DnaHash
        )))
    });

    let (tx, rx) =
        TriggerSender::new_with_loop(Duration::from_secs(5)..Duration::from_secs(30), true);

    let work_complete = publish_dht_ops_workflow(
        vault.clone(),
        Arc::new(network),
        tx,
        agent,
        ConductorTuningParams::default().min_publish_interval(),
    )
    .await
    .unwrap();

    let publish_timestamp = get_publish_time(vault, op_hash).await;

    assert_eq!(WorkComplete::Incomplete(None), work_complete);
    assert!(!rx.is_paused());
    assert!(publish_timestamp.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn workflow_handles_publish_errors() {
    holochain_trace::test_run();

    let test_db = holochain_state::test_utils::test_authored_db();
    let vault = test_db.to_db();

    let agent = fixt!(AgentPubKey);

    let op_hash = create_op(vault.clone(), agent.clone()).await.unwrap();

    let mut network = MockHolochainP2pDnaT::new();
    network.expect_publish().return_once(|_, _, _, _, _, _| {
        Err(holochain_p2p::HolochainP2pError::InvalidP2pMessage(
            "test error".to_string(),
        ))
    });

    let (tx, rx) =
        TriggerSender::new_with_loop(Duration::from_secs(5)..Duration::from_secs(30), true);

    let work_complete = publish_dht_ops_workflow(
        vault.clone(),
        Arc::new(network),
        tx,
        agent,
        ConductorTuningParams::default().min_publish_interval(),
    )
    .await
    .unwrap();

    let publish_timestamp = get_publish_time(vault, op_hash).await;

    assert_eq!(WorkComplete::Complete, work_complete);
    assert!(!rx.is_paused());
    assert!(publish_timestamp.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn retry_publish_until_receipts_received() {
    holochain_trace::test_run();

    let test_db = holochain_state::test_utils::test_authored_db();
    let vault = test_db.to_db();

    let agent = fixt!(AgentPubKey);

    let op_hash = create_op(vault.clone(), agent.clone()).await.unwrap();

    let mut network = MockHolochainP2pDnaT::new();
    network
        .expect_publish()
        .returning(|_, _, _, _, _, _| Ok(()));

    let (tx, rx) =
        TriggerSender::new_with_loop(Duration::from_secs(5)..Duration::from_secs(30), true);

    let network = Arc::new(network);

    for _ in 0..3 {
        let work_complete = publish_dht_ops_workflow(
            vault.clone(),
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

        verify_published_recently(vault.clone(), op_hash.clone()).await;
    }

    do_set_receipts_complete(vault.clone(), op_hash.clone()).await;

    let work_complete = publish_dht_ops_workflow(
        vault.clone(),
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

    let test_db = holochain_state::test_utils::test_authored_db();
    let vault = test_db.to_db();

    let agent = fixt!(AgentPubKey);

    let mut network = MockHolochainP2pDnaT::new();
    network
        .expect_publish()
        .returning(|_, _, _, _, _, _| Ok(()));

    let (tx, rx) =
        TriggerSender::new_with_loop(Duration::from_secs(5)..Duration::from_secs(30), true);

    let network = Arc::new(network);

    // Do a publish with no data to get into a paused state
    let work_complete = publish_dht_ops_workflow(
        vault.clone(),
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
    create_op(vault.clone(), agent.clone()).await.unwrap();

    let work_complete = publish_dht_ops_workflow(
        vault,
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

    let test_db = holochain_state::test_utils::test_authored_db();
    let vault = test_db.to_db();

    // Create an op for some other author
    create_op(vault.clone(), fixt!(AgentPubKey)).await.unwrap();

    let agent = fixt!(AgentPubKey);

    let mut network = MockHolochainP2pDnaT::new();
    network.expect_publish().never();

    let (tx, rx) =
        TriggerSender::new_with_loop(Duration::from_secs(5)..Duration::from_secs(30), true);

    let network = Arc::new(network);

    let work_complete = publish_dht_ops_workflow(
        vault.clone(),
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

// Even though ops are created for actions with private entries, they should not
// contain the private entry.
#[tokio::test(flavor = "multi_thread")]
async fn private_entries_are_not_published() {
    holochain_trace::test_run();

    let test_db = holochain_state::test_utils::test_authored_db();
    let vault = test_db.to_db();
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
    let new_entry_action = NewEntryAction::Create(create_action.clone());

    let register_agent_activity_op = DhtOpHashed::from_content_sync(
        ChainOp::RegisterAgentActivity(fixt!(Signature), action.clone()),
    );
    let store_entry_op = DhtOpHashed::from_content_sync(ChainOp::StoreEntry(
        fixt!(Signature),
        new_entry_action.clone(),
        fixt!(Entry),
    ));
    let store_record_op = DhtOpHashed::from_content_sync(ChainOp::StoreRecord(
        fixt!(Signature),
        action,
        RecordEntry::Hidden,
    ));

    // Double check that entry is not accessible in the RegisterAgentActivity op,
    // which does get published.
    assert!(matches!(
        register_agent_activity_op.as_chain_op().unwrap().entry(),
        RecordEntry::Hidden
    ));

    // Write all three ops to the database.
    let register_agent_activity_op_hash = register_agent_activity_op.as_hash().clone();
    let store_entry_op_hash = store_entry_op.as_hash().clone();
    let store_record_op_hash = store_record_op.as_hash().clone();
    vault
        .write_async({
            move |txn| -> StateMutationResult<()> {
                holochain_state::mutations::insert_op_authored(txn, &register_agent_activity_op)?;
                holochain_state::mutations::insert_op_authored(txn, &store_entry_op)?;
                holochain_state::mutations::insert_op_authored(txn, &store_record_op)?;
                Ok(())
            }
        })
        .await
        .unwrap();

    // RegisterAgentActivity and StoreRecord are expected to be published.
    // StoreRecord contains the entry and is expected to not be published.
    let mut network = MockHolochainP2pDnaT::new();
    let agent2 = agent.clone();
    network.expect_publish().returning(
        move |_request_validation_receipt,
              _basis_hash,
              source,
              op_hash_list,
              _timeout_ms,
              _reflect_ops| {
            assert_eq!(source, agent2);
            assert!(
                op_hash_list.contains(&register_agent_activity_op_hash)
                    || op_hash_list.contains(&store_record_op_hash)
            );
            assert!(!op_hash_list.contains(&store_entry_op_hash));
            Ok(())
        },
    );
    let network = Arc::new(network);

    let (tx, _rx) =
        TriggerSender::new_with_loop(Duration::from_secs(5)..Duration::from_secs(30), true);

    let work_complete = publish_dht_ops_workflow(
        vault.clone(),
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

async fn verify_published_recently(vault: DbWrite<DbKindAuthored>, op_hash: DhtOpHash) {
    let publish_timestamp = get_publish_time(vault.clone(), op_hash.clone())
        .await
        .expect("Expected published time to have been set");

    assert!(
        publish_timestamp
            .checked_add_signed(chrono::Duration::try_seconds(1).unwrap())
            .unwrap()
            > chrono::DateTime::<Utc>::from(SystemTime::now())
    );
}

async fn create_op(
    vault: DbWrite<DbKindAuthored>,
    author: AgentPubKey,
) -> StateMutationResult<DhtOpHash> {
    let mut create_action = fixt!(Create);
    create_action.author = author;
    let action = Action::Create(create_action);

    let op =
        DhtOpHashed::from_content_sync(ChainOp::RegisterAgentActivity(fixt!(Signature), action));

    let test_op_hash = op.as_hash().clone();
    vault
        .write_async({
            move |txn| -> StateMutationResult<()> {
                holochain_state::mutations::insert_op_authored(txn, &op)?;
                Ok(())
            }
        })
        .await
        .unwrap();

    Ok(test_op_hash)
}

async fn get_publish_time(
    vault: DbWrite<DbKindAuthored>,
    op_hash: DhtOpHash,
) -> Option<chrono::DateTime<Utc>> {
    vault
        .read_async(
            move |txn| -> DatabaseResult<Option<chrono::DateTime<Utc>>> {
                let time: Option<i64> = txn.query_row(
                    "SELECT last_publish_time FROM DhtOp WHERE hash = :hash",
                    named_params! {
                        ":hash": op_hash,
                    },
                    |row| row.get(0),
                )?;

                Ok(time.and_then(|t| chrono::DateTime::from_timestamp(t, 0)))
            },
        )
        .await
        .unwrap()
}

async fn do_set_receipts_complete(vault: DbWrite<DbKindAuthored>, op_hash: DhtOpHash) {
    vault
        .write_async({
            move |txn| -> StateMutationResult<()> {
                set_receipts_complete(txn, &op_hash, true)?;
                Ok(())
            }
        })
        .await
        .unwrap();
}
