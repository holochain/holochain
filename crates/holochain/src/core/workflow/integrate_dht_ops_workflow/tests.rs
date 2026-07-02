use super::*;
use crate::core::queue_consumer::TriggerSender;
use crate::core::workflow::provider::authored_db_provider::MockAuthoredDbProvider;
use ::fixt::prelude::*;
use holo_hash::fixt::{AgentPubKeyFixturator, DnaHashFixturator};
use holo_hash::{AgentPubKey, DhtOpHash, DnaHash};
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_sqlite::db::DbKindAuthored;
use holochain_sqlite::error::{DatabaseError, DatabaseResult};
use holochain_state::dht_store::DhtStore;
use holochain_state::mutations;
use holochain_state::prelude::SysOutcome;
use holochain_state::test_utils::{test_authored_db_with_id, test_dht_store, TestDb};
use holochain_types::prelude::{ChainOp, DhtOp, DhtOpHashed};
use kitsune2_api::StoredOp;
use must_future::MustBoxFuture;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

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
                let expected_op = StoredOp {
                    op_id: op.to_hash().to_located_k2_op_id(&op.dht_basis()),
                    created_at: kitsune2_api::Timestamp::from_micros(op.timestamp().as_micros()),
                };
                assert_eq!(ops, vec![expected_op], "test case {i}");
                Ok(())
            });
        let hc_p2p = Arc::new(hc_p2p);
        integrate_dht_ops_workflow(dht_store, tx, hc_p2p, mock_authored_db_provider_none())
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
    integrate_dht_ops_workflow(dht_store, tx, hc_p2p, mock_authored_db_provider_none())
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn single_local_author_marks_both_databases() {
    holochain_trace::test_run();
    let dna_hash = fixt!(DnaHash);
    let author = fixt!(AgentPubKey);
    let (_op, hashed) = make_store_entry_op(author.clone());

    let dht_store = test_dht_store(dna_hash.clone()).await;
    insert_validated_op_to_store(&dht_store, &hashed).await;

    let authored_db = Arc::new(test_authored_db_with_id(1));

    // Insert the same op into the authored DB so it can be marked as integrated
    authored_db
        .to_db()
        .write_async({
            let hashed = hashed.clone();
            move |txn| -> DatabaseResult<()> {
                mutations::insert_op_authored(txn, &hashed)
                    .map_err(|e| DatabaseError::Other(e.into()))
            }
        })
        .await
        .unwrap();

    let (mock, _, _) = mock_authored_db_provider_with_db(
        dna_hash.clone(),
        vec![(author.clone(), Arc::clone(&authored_db))],
    );

    let (tx, _rx) = TriggerSender::new();
    let mut hc_p2p = MockHolochainP2pDnaT::new();
    hc_p2p.expect_dna_hash().return_const(dna_hash.clone());
    hc_p2p.expect_new_integrated_data().return_once(move |ops| {
        assert_eq!(ops.len(), 1);
        Ok(())
    });
    let mock_network = Arc::new(hc_p2p);

    integrate_dht_ops_workflow(dht_store.clone(), tx, mock_network, mock)
        .await
        .unwrap();

    let hash = hashed.as_hash().clone();
    assert!(
        dht_store.when_integrated(&hash).await.unwrap().is_some(),
        "Op should be marked as integrated in DHT store"
    );
    assert!(
        authored_when_integrated(&authored_db, &hash)
            .await
            .is_some(),
        "Op should be marked as integrated in authored database"
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

    let authored_a = Arc::new(test_authored_db_with_id(1));
    let authored_b = Arc::new(test_authored_db_with_id(2));

    // Insert ops into their respective authored DBs
    authored_a
        .to_db()
        .write_async({
            let hashed = hashed_a.clone();
            move |txn| -> DatabaseResult<()> {
                mutations::insert_op_authored(txn, &hashed)
                    .map_err(|e| DatabaseError::Other(e.into()))
            }
        })
        .await
        .unwrap();

    authored_b
        .to_db()
        .write_async({
            let hashed = hashed_b.clone();
            move |txn| -> DatabaseResult<()> {
                mutations::insert_op_authored(txn, &hashed)
                    .map_err(|e| DatabaseError::Other(e.into()))
            }
        })
        .await
        .unwrap();

    let (mock, _, _) = mock_authored_db_provider_with_db(
        dna_hash.clone(),
        vec![
            (author_a.clone(), authored_a.clone()),
            (author_b.clone(), authored_b.clone()),
        ],
    );

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

    integrate_dht_ops_workflow(dht_store.clone(), tx, mock_network, mock)
        .await
        .unwrap();

    let hash_a = hashed_a.as_hash().clone();
    let hash_b = hashed_b.as_hash().clone();
    assert!(authored_when_integrated(&authored_a, &hash_a)
        .await
        .is_some());
    assert!(authored_when_integrated(&authored_b, &hash_b)
        .await
        .is_some());
    assert!(dht_store.when_integrated(&hash_a).await.unwrap().is_some());
    assert!(dht_store.when_integrated(&hash_b).await.unwrap().is_some());
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn mock_authored_db_provider_none() -> Arc<MockAuthoredDbProvider> {
    let mut mock = MockAuthoredDbProvider::new();
    mock.expect_get_authored_db()
        .returning(|_, _| MustBoxFuture::new(async { Ok(None) }));
    Arc::new(mock)
}

// Type alias to simplify the complex return type
type MockProviderWithState = (
    Arc<dyn super::provider::authored_db_provider::AuthoredDbProvider>,
    Arc<Mutex<HashMap<AgentPubKey, Arc<TestDb<DbKindAuthored>>>>>,
    Arc<AtomicUsize>,
);

fn mock_authored_db_provider_with_db(
    dna_hash: DnaHash,
    authors: Vec<(AgentPubKey, Arc<TestDb<DbKindAuthored>>)>,
) -> MockProviderWithState {
    let mut mock = MockAuthoredDbProvider::new();
    let initial: HashMap<_, _> = authors.into_iter().collect();
    let state = Arc::new(Mutex::new(initial));
    let lookup_count = Arc::new(AtomicUsize::new(0));
    let state_clone = Arc::clone(&state);
    let count_clone = Arc::clone(&lookup_count);
    let dna_hash_for_mock = dna_hash.clone();
    mock.expect_get_authored_db()
        .returning(move |requested_dna, requested_author| {
            count_clone.fetch_add(1, Ordering::SeqCst);
            let dna_hash_clone = dna_hash_for_mock.clone();
            let state_inner = Arc::clone(&state_clone);
            let requested_dna = requested_dna.clone();
            let requested_author = requested_author.clone();
            MustBoxFuture::new(async move {
                if requested_dna != dna_hash_clone {
                    return Ok(None);
                }
                let guard = state_inner.lock().unwrap();
                Ok(guard.get(&requested_author).map(|db| db.to_db()))
            })
        });
    (Arc::new(mock), state, lookup_count)
}

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
    let op: DhtOp = ChainOp::StoreEntry(fixt!(Signature), action.clone().into(), entry).into();
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
    let op: DhtOp =
        ChainOp::StoreRecord(fixt!(Signature), action.clone().into(), entry.into()).into();
    let hashed = DhtOpHashed::from_content_sync(op.clone());
    (op, hashed)
}

async fn authored_when_integrated(
    db: &TestDb<DbKindAuthored>,
    hash: &DhtOpHash,
) -> Option<Timestamp> {
    use holochain_sqlite::rusqlite::named_params;
    use holochain_sqlite::rusqlite::OptionalExtension;
    db.to_db()
        .read_async({
            let hash = hash.clone();
            move |txn| -> DatabaseResult<Option<Timestamp>> {
                txn.query_row(
                    "SELECT when_integrated FROM DhtOp WHERE hash = :hash",
                    named_params! { ":hash": hash },
                    |row| row.get(0),
                )
                .optional()
                .map_err(DatabaseError::from)
            }
        })
        .await
        .unwrap()
}
