use super::*;
use holo_hash::{ActionHash, AnyLinkableHash, DnaHash, EntryHash};
use holochain_types::dht_op::{ChainOp, DhtOp, DhtOpHashed, RenderedOp, RenderedOps};
use holochain_types::prelude::Signature;
use holochain_zome_types::action::{Action, Create, CreateLink, DeleteLink, EntryType};
use holochain_zome_types::entry_def::EntryVisibility;
use holochain_zome_types::op::ChainOpType;
use holochain_zome_types::prelude::AppEntryDef;
use holochain_zome_types::validate::ValidationStatus;
use std::sync::Arc;

fn dht_id() -> Dht {
    Dht::new(Arc::new(DnaHash::from_raw_36(vec![0u8; 36])))
}

fn agent(seed: u8) -> AgentPubKey {
    AgentPubKey::from_raw_36(vec![seed; 36])
}

#[tokio::test]
async fn delete_live_ephemeral_scheduled_functions_roundtrip() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let author = agent(1);

    // Insert an ephemeral row with start_at <= now so it is "live".
    store
        .db
        .upsert_scheduled_function(InsertScheduledFunction {
            author: &author,
            zome_name: "z",
            scheduled_fn: "f",
            maybe_schedule: b"",
            start_at: Timestamp::from_micros(50),
            end_at: Timestamp::from_micros(300),
            ephemeral: true,
        })
        .await
        .unwrap();

    let deleted = store
        .delete_live_ephemeral_scheduled_functions(&author, Timestamp::from_micros(100))
        .await
        .unwrap();
    assert_eq!(deleted, 1);

    // A second call should delete nothing.
    let deleted2 = store
        .delete_live_ephemeral_scheduled_functions(&author, Timestamp::from_micros(100))
        .await
        .unwrap();
    assert_eq!(deleted2, 0);
}

#[tokio::test]
async fn upsert_scheduled_function_none_schedule_writes_ephemeral_row() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let author = agent(2);

    // Seed a persisted row (ephemeral=false, start=0, end=100).
    store
        .db
        .upsert_scheduled_function(InsertScheduledFunction {
            author: &author,
            zome_name: "z",
            scheduled_fn: "f",
            maybe_schedule: b"",
            start_at: Timestamp::from_micros(0),
            end_at: Timestamp::from_micros(100),
            ephemeral: false,
        })
        .await
        .unwrap();

    // Upsert with None maps to (now, max, ephemeral=true). With now=50,
    // the row should be rewritten to (start=50, end=max, ephemeral=true).
    let rows = store
        .upsert_scheduled_function(
            &author,
            &ScheduledFn::new("z".into(), "f".into()),
            &None,
            Timestamp::from_micros(50),
        )
        .await
        .unwrap();
    assert_eq!(rows, 1, "None upsert should write exactly one row");

    // Confirm the row is now ephemeral and live at now=60:
    // delete_live_ephemeral removes ephemeral rows with start_at <= now.
    let deleted = store
        .db
        .delete_live_ephemeral_scheduled_functions(&author, Timestamp::from_micros(60))
        .await
        .unwrap();
    assert_eq!(
        deleted, 1,
        "row should be ephemeral with start_at <= 60 after None upsert"
    );
}

#[tokio::test]
async fn mark_chain_op_receipts_complete_no_row() {
    // No matching ChainOpPublish row → ChainOpPublishMissing error.
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let op_hash = DhtOpHash::from_raw_36(vec![1u8; 36]);

    let err = store
        .mark_chain_op_receipts_complete(&op_hash)
        .await
        .unwrap_err();
    assert!(matches!(err, DhtStoreError::ChainOpPublishMissing));
}

#[tokio::test]
async fn purge_all_empties_every_table() {
    // Seed a row in each independent table that doesn't need an Action FK,
    // call purge_all, and confirm every table is empty.
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let author = AgentPubKey::from_raw_36(vec![1u8; 36]);

    // ScheduledFunction.
    store
        .db
        .upsert_scheduled_function(InsertScheduledFunction {
            author: &author,
            zome_name: "z",
            scheduled_fn: "f",
            maybe_schedule: b"",
            start_at: Timestamp::from_micros(1),
            end_at: Timestamp::from_micros(2),
            ephemeral: true,
        })
        .await
        .unwrap();

    store.purge_all().await.unwrap();

    let pool = store.db.pool();
    for (table, sql) in [
        ("Action", "SELECT COUNT(*) FROM Action"),
        ("Entry", "SELECT COUNT(*) FROM Entry"),
        ("PrivateEntry", "SELECT COUNT(*) FROM PrivateEntry"),
        ("CapGrant", "SELECT COUNT(*) FROM CapGrant"),
        ("CapClaim", "SELECT COUNT(*) FROM CapClaim"),
        ("ChainLock", "SELECT COUNT(*) FROM ChainLock"),
        ("LimboChainOp", "SELECT COUNT(*) FROM LimboChainOp"),
        ("LimboWarrantOp", "SELECT COUNT(*) FROM LimboWarrantOp"),
        ("ChainOp", "SELECT COUNT(*) FROM ChainOp"),
        ("ChainOpPublish", "SELECT COUNT(*) FROM ChainOpPublish"),
        (
            "ValidationReceipt",
            "SELECT COUNT(*) FROM ValidationReceipt",
        ),
        ("Warrant", "SELECT COUNT(*) FROM Warrant"),
        ("WarrantOp", "SELECT COUNT(*) FROM WarrantOp"),
        ("WarrantPublish", "SELECT COUNT(*) FROM WarrantPublish"),
        ("Link", "SELECT COUNT(*) FROM Link"),
        ("DeletedLink", "SELECT COUNT(*) FROM DeletedLink"),
        ("UpdatedRecord", "SELECT COUNT(*) FROM UpdatedRecord"),
        ("DeletedRecord", "SELECT COUNT(*) FROM DeletedRecord"),
        (
            "ScheduledFunction",
            "SELECT COUNT(*) FROM ScheduledFunction",
        ),
        ("SliceHash", "SELECT COUNT(*) FROM SliceHash"),
    ] {
        let count: i64 = sqlx::query_scalar(sql).fetch_one(pool).await.unwrap();
        assert_eq!(count, 0, "{table} not empty after purge_all");
    }
}

/// Build a `StoreRecord` chain op for a `Create` action carrying a public
/// entry.  `seed` is used to make each call produce distinct keys /
/// hashes (it drives the raw bytes of the author key and entry hash).
fn build_test_store_record_op_hashed(seed: u8) -> DhtOpHashed {
    use holo_hash::{ActionHash, EntryHash};
    use holochain_serialized_bytes::UnsafeBytes;
    use holochain_types::prelude::{AppEntryBytes, Entry, RecordEntry, Signature};
    use holochain_zome_types::action::{Action, Create, EntryType};
    use holochain_zome_types::entry_def::EntryVisibility;
    use holochain_zome_types::prelude::AppEntryDef;

    let author = AgentPubKey::from_raw_36(vec![seed; 36]);
    let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
    let entry = Entry::App(AppEntryBytes(
        holochain_serialized_bytes::SerializedBytes::from(UnsafeBytes::from(vec![seed; 8])),
    ));
    let sig = Signature::from([seed; 64]);
    let action = Action::Create(Create {
        author: author.clone(),
        timestamp: Timestamp::from_micros(seed as i64 * 1000),
        action_seq: 1,
        prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(200); 36]),
        entry_type: EntryType::App(AppEntryDef::new(
            0.into(),
            0.into(),
            EntryVisibility::Public,
        )),
        entry_hash: entry_hash.clone(),
        weight: Default::default(),
    });
    let op = DhtOp::ChainOp(Box::new(ChainOp::StoreRecord(
        sig,
        action,
        RecordEntry::Present(entry),
    )));
    DhtOpHashed::from_content_sync(op)
}

/// Build a `WarrantOp` (`ChainIntegrityWarrant::InvalidChainOp`) for
/// testing.  `seed` drives distinct key bytes.
fn build_test_warrant_op_hashed(seed: u8) -> DhtOpHashed {
    use holochain_types::warrant::WarrantOp;
    use holochain_zome_types::op::ChainOpType;
    use holochain_zome_types::prelude::{
        ChainIntegrityWarrant, Signature, SignedWarrant, Warrant, WarrantProof,
    };

    let action_author = AgentPubKey::from_raw_36(vec![seed; 36]);
    let warrantee = AgentPubKey::from_raw_36(vec![seed.wrapping_add(50); 36]);
    let action_hash = holo_hash::ActionHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
    let warrant = SignedWarrant::new(
        Warrant::new(
            WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
                action_author: action_author.clone(),
                action: (action_hash, Signature::from([seed; 64])),
                chain_op_type: ChainOpType::StoreRecord,
                reason: "test warrant".into(),
            }),
            AgentPubKey::from_raw_36(vec![seed.wrapping_add(10); 36]),
            Timestamp::from_micros(seed as i64 * 1000),
            warrantee,
        ),
        Signature::from([seed.wrapping_add(1); 64]),
    );
    let op = DhtOp::WarrantOp(Box::new(WarrantOp::from(warrant)));
    DhtOpHashed::from_content_sync(op)
}

#[tokio::test]
async fn record_incoming_ops_inserts_limbo_chain_op() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let op = build_test_store_record_op_hashed(1);
    let op_hash = op.as_hash().clone();

    // Extract the action hash before consuming `op`.
    let action_hash = {
        let action = op.as_content().as_chain_op().unwrap().action();
        holo_hash::ActionHash::with_data_sync(&action)
    };

    store.record_incoming_ops(vec![op]).await.unwrap();

    // Action row was inserted.
    let found = store.db.as_ref().get_action(action_hash).await.unwrap();
    assert!(
        found.is_some(),
        "Action row not found after record_incoming_ops"
    );

    // LimboChainOp row has require_receipt=true and a positive serialized_size.
    let row = store
        .db
        .as_ref()
        .get_limbo_chain_op(op_hash)
        .await
        .unwrap()
        .expect("LimboChainOp row not found");
    assert_eq!(row.require_receipt, 1, "require_receipt should be 1 (true)");
    assert!(row.serialized_size > 0, "serialized_size should be > 0");
}

#[tokio::test]
async fn record_incoming_ops_inserts_limbo_warrant() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let warrant_op = build_test_warrant_op_hashed(1);
    let op_hash = warrant_op.as_hash().clone();

    store.record_incoming_ops(vec![warrant_op]).await.unwrap();

    let row = store.db.as_ref().get_limbo_warrant(op_hash).await.unwrap();
    assert!(
        row.is_some(),
        "LimboWarrant row not found after record_incoming_ops"
    );
    let row = row.unwrap();
    assert!(row.serialized_size > 0, "serialized_size should be > 0");
    // The rejection reason is extracted from the warrant proof and stored in
    // its own column.
    assert_eq!(row.reason.as_deref(), Some("test warrant"));
}

#[tokio::test]
async fn record_sys_validation_outcome_chain_op() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();

    // Seed a LimboChainOp row by calling record_incoming_ops (reusing the C1 helper).
    let op = build_test_store_record_op_hashed(10);
    let op_hash = op.as_hash().clone();
    store.record_incoming_ops(vec![op]).await.unwrap();

    // Confirm sys_validation_status starts as NULL.
    let row_before = store
        .db
        .as_ref()
        .get_limbo_chain_op(op_hash.clone())
        .await
        .unwrap()
        .expect("LimboChainOp row not found after seed");
    assert_eq!(row_before.sys_validation_status, None);

    store
        .record_chain_op_sys_validation_outcomes(vec![(op_hash.clone(), SysOutcome::Accepted)])
        .await
        .unwrap();

    let row = store
        .db
        .as_ref()
        .get_limbo_chain_op(op_hash)
        .await
        .unwrap()
        .expect("LimboChainOp row not found after update");
    assert_eq!(row.sys_validation_status, Some(1));
}

#[tokio::test]
async fn record_sys_validation_outcome_warrant() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();

    // Seed a LimboWarrant row by calling record_incoming_ops (reusing the C1 helper).
    let op = build_test_warrant_op_hashed(20);
    let op_hash = op.as_hash().clone();
    store.record_incoming_ops(vec![op]).await.unwrap();

    // Confirm sys_validation_status starts as NULL.
    let row_before = store
        .db
        .as_ref()
        .get_limbo_warrant(op_hash.clone())
        .await
        .unwrap()
        .expect("LimboWarrant row not found after seed");
    assert_eq!(row_before.sys_validation_status, None);

    store
        .record_warrant_sys_validation_outcomes(vec![(op_hash.clone(), SysOutcome::Rejected)])
        .await
        .unwrap();

    let row = store
        .db
        .as_ref()
        .get_limbo_warrant(op_hash)
        .await
        .unwrap()
        .expect("LimboWarrant row not found after update");
    assert_eq!(row.sys_validation_status, Some(2));
}

#[tokio::test]
async fn record_app_validation_outcome_accepted() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let op = build_test_store_record_op_hashed(11);
    store.record_incoming_ops(vec![op.clone()]).await.unwrap();
    // sys_validation_status must be set before app (ordering constraint).
    store
        .record_chain_op_sys_validation_outcomes(vec![(op.as_hash().clone(), SysOutcome::Accepted)])
        .await
        .unwrap();
    // Pre-state: app_validation_status should be NULL.
    let row = store
        .db()
        .as_ref()
        .get_limbo_chain_op(op.as_hash().clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.app_validation_status, None);

    store
        .record_app_validation_outcomes(vec![(op.as_hash().clone(), AppOutcome::Accepted)])
        .await
        .unwrap();

    let row = store
        .db()
        .as_ref()
        .get_limbo_chain_op(op.as_hash().clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.app_validation_status, Some(1));
}

#[tokio::test]
async fn record_app_validation_outcome_rejected() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let op = build_test_store_record_op_hashed(12);
    store.record_incoming_ops(vec![op.clone()]).await.unwrap();
    // sys_validation_status must be set before app (ordering constraint).
    store
        .record_chain_op_sys_validation_outcomes(vec![(op.as_hash().clone(), SysOutcome::Accepted)])
        .await
        .unwrap();

    store
        .record_app_validation_outcomes(vec![(op.as_hash().clone(), AppOutcome::Rejected)])
        .await
        .unwrap();

    let row = store
        .db()
        .as_ref()
        .get_limbo_chain_op(op.as_hash().clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.app_validation_status, Some(2));
}

#[tokio::test]
async fn record_incoming_ops_dedupes_on_conflict() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let op = build_test_store_record_op_hashed(2);
    let op_hash = op.as_hash().clone();

    // First insert.
    store.record_incoming_ops(vec![op.clone()]).await.unwrap();
    // Re-insert: ON CONFLICT IGNORE means no error and no duplicate row.
    store.record_incoming_ops(vec![op]).await.unwrap();

    // Exactly one row still exists.
    let row = store.db.as_ref().get_limbo_chain_op(op_hash).await.unwrap();
    assert!(row.is_some(), "LimboChainOp row should still be present");
}

#[tokio::test]
async fn integrate_ready_ops_promotes_ready_chain_op() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let op = build_test_store_record_op_hashed(50);
    store.record_incoming_ops(vec![op.clone()]).await.unwrap();
    // Mark ready: sys=1, app=1.
    store
        .record_chain_op_sys_validation_outcomes(vec![(op.as_hash().clone(), SysOutcome::Accepted)])
        .await
        .unwrap();
    store
        .record_app_validation_outcomes(vec![(op.as_hash().clone(), AppOutcome::Accepted)])
        .await
        .unwrap();

    let summaries = store
        .integrate_ready_ops(Timestamp::from_micros(999))
        .await
        .unwrap();
    let promoted_hashes: Vec<_> = summaries.iter().map(|s| s.op_hash.clone()).collect();
    assert_eq!(promoted_hashes, vec![op.as_hash().clone()]);

    assert!(store
        .db()
        .as_ref()
        .get_limbo_chain_op(op.as_hash().clone())
        .await
        .unwrap()
        .is_none());
    let row = store
        .db()
        .as_ref()
        .get_chain_op(op.as_hash().clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.when_integrated, 999);
    assert_eq!(
        row.validation_status,
        i64::from(holochain_zome_types::dht_v2::RecordValidity::Accepted)
    );
}

#[tokio::test]
async fn integrate_ready_ops_skips_unready() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let op = build_test_store_record_op_hashed(51);
    store.record_incoming_ops(vec![op.clone()]).await.unwrap();
    // No validation outcomes recorded — sys/app are NULL, not ready.
    let summaries = store
        .integrate_ready_ops(Timestamp::from_micros(999))
        .await
        .unwrap();
    assert!(summaries.is_empty());

    // Op still in limbo, not in ChainOp.
    assert!(store
        .db()
        .as_ref()
        .get_limbo_chain_op(op.as_hash().clone())
        .await
        .unwrap()
        .is_some());
    assert!(store
        .db()
        .as_ref()
        .get_chain_op(op.as_hash().clone())
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn integrate_ready_ops_promotes_warrant() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let warrant = build_test_warrant_op_hashed(52);
    store
        .record_incoming_ops(vec![warrant.clone()])
        .await
        .unwrap();
    // Mark sys=1 (warrants have no app validation).
    store
        .record_warrant_sys_validation_outcomes(vec![(
            warrant.as_hash().clone(),
            SysOutcome::Accepted,
        )])
        .await
        .unwrap();

    let summaries = store
        .integrate_ready_ops(Timestamp::from_micros(999))
        .await
        .unwrap();
    let promoted_hashes: Vec<_> = summaries.iter().map(|s| s.op_hash.clone()).collect();
    assert_eq!(promoted_hashes, vec![warrant.as_hash().clone()]);

    assert!(store
        .db()
        .as_ref()
        .get_limbo_warrant(warrant.as_hash().clone())
        .await
        .unwrap()
        .is_none());
    assert!(store
        .db()
        .as_ref()
        .get_warrant(warrant.as_hash().clone())
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn record_validation_receipt_inserts_and_counts() {
    use holochain_types::prelude::Signature;
    use holochain_types::prelude::{SignedValidationReceipt, ValidationReceipt, ValidationStatus};

    let store = DhtStore::new_test(dht_id()).await.unwrap();

    // Seed a chain op and promote it to ChainOp so the FK is satisfied.
    let op = build_test_store_record_op_hashed(60);
    store.record_incoming_ops(vec![op.clone()]).await.unwrap();
    store
        .record_chain_op_sys_validation_outcomes(vec![(op.as_hash().clone(), SysOutcome::Accepted)])
        .await
        .unwrap();
    store
        .record_app_validation_outcomes(vec![(op.as_hash().clone(), AppOutcome::Accepted)])
        .await
        .unwrap();
    store
        .integrate_ready_ops(Timestamp::from_micros(1))
        .await
        .unwrap();

    let receipt = SignedValidationReceipt {
        receipt: ValidationReceipt {
            dht_op_hash: op.as_hash().clone(),
            validation_status: ValidationStatus::Valid,
            validators: vec![AgentPubKey::from_raw_36(vec![5u8; 36])],
            when_integrated: Timestamp::from_micros(1),
        },
        validators_signatures: vec![Signature([0u8; 64])],
    };

    let count = store.record_validation_receipt(&receipt).await.unwrap();
    assert_eq!(count, 1);

    // Inserting the same receipt again should be a no-op (ON CONFLICT IGNORE)
    // and return count of 1 again.
    let count = store.record_validation_receipt(&receipt).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn apply_countersigning_success_clears_withhold() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();

    // Seed an op through the full pipeline into ChainOp (satisfies FK for ChainOpPublish).
    let op = build_test_store_record_op_hashed(80);
    store.record_incoming_ops(vec![op.clone()]).await.unwrap();
    store
        .record_chain_op_sys_validation_outcomes(vec![(op.as_hash().clone(), SysOutcome::Accepted)])
        .await
        .unwrap();
    store
        .record_app_validation_outcomes(vec![(op.as_hash().clone(), AppOutcome::Accepted)])
        .await
        .unwrap();
    store
        .integrate_ready_ops(Timestamp::from_micros(1))
        .await
        .unwrap();

    // Seed ChainOpPublish with withhold_publish = Some(true).
    store
        .db()
        .insert_chain_op_publish(op.as_hash(), None, None, Some(true))
        .await
        .unwrap();
    let row = store
        .db()
        .as_ref()
        .get_chain_op_publish(op.as_hash().clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.withhold_publish, Some(1));

    store
        .clear_op_withhold_publishes(vec![op.as_hash().clone()])
        .await
        .unwrap();

    let row = store
        .db()
        .as_ref()
        .get_chain_op_publish(op.as_hash().clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.withhold_publish, None);
}

#[tokio::test]
async fn apply_countersigning_success_no_op_when_row_absent() {
    // No ChainOpPublish row exists — method should not error.
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let dummy_hash = DhtOpHash::from_raw_36(vec![0xAA; 36]);
    store
        .clear_op_withhold_publishes(vec![dummy_hash])
        .await
        .unwrap();
}

#[tokio::test]
async fn record_published_op_hashes_updates_publish_time() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    // Seed an op in ChainOp via the standard pipeline.
    let op = build_test_store_record_op_hashed(90);
    store.record_incoming_ops(vec![op.clone()]).await.unwrap();
    store
        .record_chain_op_sys_validation_outcomes(vec![(op.as_hash().clone(), SysOutcome::Accepted)])
        .await
        .unwrap();
    store
        .record_app_validation_outcomes(vec![(op.as_hash().clone(), AppOutcome::Accepted)])
        .await
        .unwrap();
    store
        .integrate_ready_ops(Timestamp::from_micros(1))
        .await
        .unwrap();

    // Insert a ChainOpPublish row with NULL last_publish_time.
    store
        .db()
        .insert_chain_op_publish(op.as_hash(), None, None, None)
        .await
        .unwrap();

    store
        .record_published_op_hashes(vec![op.as_hash().clone()], Timestamp::from_micros(42))
        .await
        .unwrap();

    let row = store
        .db()
        .as_ref()
        .get_chain_op_publish(op.as_hash().clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.last_publish_time, Some(42));
}

#[tokio::test]
async fn reject_chain_op_rejects_integrated_op() {
    use holochain_zome_types::dht_v2::RecordValidity;
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let op = build_test_store_record_op_hashed(100);
    store.record_incoming_ops(vec![op.clone()]).await.unwrap();
    store
        .record_chain_op_sys_validation_outcomes(vec![(op.as_hash().clone(), SysOutcome::Accepted)])
        .await
        .unwrap();
    store
        .record_app_validation_outcomes(vec![(op.as_hash().clone(), AppOutcome::Accepted)])
        .await
        .unwrap();
    store
        .integrate_ready_ops(Timestamp::from_micros(1))
        .await
        .unwrap();
    // Simulate a network-cached op: clear the locally_validated flag set by
    // promotion. The reject_chain_ops path only changes status on network-
    // cached ops.
    sqlx::query("UPDATE ChainOp SET locally_validated = 0 WHERE hash = ?")
        .bind(op.as_hash().get_raw_36())
        .execute(store.db().pool())
        .await
        .unwrap();

    store
        .reject_chain_ops(vec![op.as_hash().clone()])
        .await
        .unwrap();

    let row = store
        .db()
        .as_ref()
        .get_chain_op(op.as_hash().clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.validation_status, i64::from(RecordValidity::Rejected));
}

#[tokio::test]
async fn reject_chain_op_no_op_for_locally_validated_integrated_op() {
    use holochain_zome_types::dht_v2::RecordValidity;
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let op = build_test_store_record_op_hashed(102);
    store.record_incoming_ops(vec![op.clone()]).await.unwrap();
    store
        .record_chain_op_sys_validation_outcomes(vec![(op.as_hash().clone(), SysOutcome::Accepted)])
        .await
        .unwrap();
    store
        .record_app_validation_outcomes(vec![(op.as_hash().clone(), AppOutcome::Accepted)])
        .await
        .unwrap();
    store
        .integrate_ready_ops(Timestamp::from_micros(1))
        .await
        .unwrap();

    // Promotion sets locally_validated = 1; reject_chain_ops should be a
    // silent no-op for locally-validated integrated ops.
    store
        .reject_chain_ops(vec![op.as_hash().clone()])
        .await
        .unwrap();

    let row = store
        .db()
        .as_ref()
        .get_chain_op(op.as_hash().clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.validation_status, i64::from(RecordValidity::Accepted));
}

#[tokio::test]
async fn reject_chain_op_rejects_limbo_op() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let op = build_test_store_record_op_hashed(101);
    store.record_incoming_ops(vec![op.clone()]).await.unwrap();
    // Op is in limbo with sys=NULL, app=NULL.

    store
        .reject_chain_ops(vec![op.as_hash().clone()])
        .await
        .unwrap();

    // sys=NULL prior to reject → sys=Rejected, app=NULL.
    let row = store
        .db()
        .as_ref()
        .get_limbo_chain_op(op.as_hash().clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.sys_validation_status, Some(2));
    assert_eq!(row.app_validation_status, None);
}

/// Build a single-op `RenderedOps` for a `StoreRecord(Create)` using the
/// same style as `cache.rs` tests.  Returns `(RenderedOps, action_hash)`.
fn build_rendered_store_record_for_move(seed: u8) -> (RenderedOps, holo_hash::ActionHash) {
    use holo_hash::{ActionHash, EntryHash};
    use holochain_serialized_bytes::UnsafeBytes;
    use holochain_types::prelude::{AppEntryBytes, Entry, EntryHashed};

    let author = AgentPubKey::from_raw_36(vec![seed; 36]);
    let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
    let entry = Entry::App(AppEntryBytes(
        holochain_serialized_bytes::SerializedBytes::from(UnsafeBytes::from(vec![seed; 8])),
    ));
    let sig = Signature::from([seed; 64]);
    let action = Action::Create(Create {
        author,
        timestamp: Timestamp::from_micros(seed as i64 * 1000),
        action_seq: 1,
        prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(200); 36]),
        entry_type: EntryType::App(AppEntryDef::new(
            0.into(),
            0.into(),
            EntryVisibility::Public,
        )),
        entry_hash: entry_hash.clone(),
        weight: Default::default(),
    });
    let entry_hashed = EntryHashed::with_pre_hashed(entry, entry_hash);
    let rendered =
        RenderedOp::new(action, sig, None, ChainOpType::StoreRecord).expect("rendered op build");
    let action_hash = rendered.action.as_hash().clone();
    let ops = RenderedOps {
        entry: Some(entry_hashed),
        ops: vec![rendered],
        warrant: None,
    };
    (ops, action_hash)
}

#[tokio::test]
async fn move_warranted_op_to_limbo_moves_locally_validated_false() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let (rendered, action_hash) = build_rendered_store_record_for_move(42);
    let op_hash = rendered.ops[0].op_hash.clone();

    // Cache the op: inserts into ChainOp with locally_validated = 0.
    store.cache_chain_ops(&rendered).await.unwrap();

    // Confirm the op is in ChainOp with locally_validated = 0.
    let chain_row = store
        .db()
        .as_ref()
        .get_chain_op(op_hash.clone())
        .await
        .unwrap()
        .expect("ChainOp row missing after cache_chain_ops");
    assert_eq!(chain_row.locally_validated, 0);

    // Move to limbo.
    let moved = store
        .move_warranted_op_to_limbo(&action_hash, ChainOpType::StoreRecord)
        .await
        .unwrap();
    assert!(moved, "expected row to be moved");

    // ChainOp row should be gone.
    let chain_row_after = store
        .db()
        .as_ref()
        .get_chain_op(op_hash.clone())
        .await
        .unwrap();
    assert!(
        chain_row_after.is_none(),
        "ChainOp row should be removed after move_warranted_op_to_limbo"
    );

    // LimboChainOp row should exist with cleared validation status.
    let limbo_row = store
        .db()
        .as_ref()
        .get_limbo_chain_op(op_hash)
        .await
        .unwrap()
        .expect("LimboChainOp row missing after move_warranted_op_to_limbo");
    assert_eq!(
        limbo_row.sys_validation_status, None,
        "sys_validation_status should be NULL"
    );
    assert_eq!(
        limbo_row.app_validation_status, None,
        "app_validation_status should be NULL"
    );
    assert_eq!(limbo_row.require_receipt, 0);
}

#[tokio::test]
async fn move_warranted_op_to_limbo_returns_false_when_not_cached() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let action_hash = holo_hash::ActionHash::from_raw_36(vec![0xBB; 36]);

    let moved = store
        .move_warranted_op_to_limbo(&action_hash, ChainOpType::StoreRecord)
        .await
        .unwrap();
    assert!(!moved, "expected false when no matching cached row exists");
}

#[tokio::test]
async fn move_warranted_op_to_limbo_no_match_for_locally_validated_true() {
    // An op that is locally validated (locally_validated = 1 via incoming ops path)
    // should NOT be moved, because the predicate requires locally_validated = 0.
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let op = build_test_store_record_op_hashed(43);
    let op_hash = op.as_hash().clone();
    let action_hash = {
        let action = op.as_content().as_chain_op().unwrap().action();
        holo_hash::ActionHash::with_data_sync(&action)
    };

    // record_incoming_ops → LimboChainOp (not ChainOp), then promote to ChainOp
    // with locally_validated = 1.
    store.record_incoming_ops(vec![op]).await.unwrap();
    store
        .record_chain_op_sys_validation_outcomes(vec![(op_hash.clone(), SysOutcome::Accepted)])
        .await
        .unwrap();
    store
        .record_app_validation_outcomes(vec![(op_hash.clone(), AppOutcome::Accepted)])
        .await
        .unwrap();
    store
        .integrate_ready_ops(Timestamp::from_micros(1))
        .await
        .unwrap();

    // ChainOp now has locally_validated = 1. The move should not match it.
    let moved = store
        .move_warranted_op_to_limbo(&action_hash, ChainOpType::StoreRecord)
        .await
        .unwrap();
    assert!(!moved, "should not move a locally_validated = 1 row");

    // Verify the row is still in ChainOp.
    let row = store.db().as_ref().get_chain_op(op_hash).await.unwrap();
    assert!(row.is_some(), "ChainOp row should still be present");
}

/// Like `build_test_store_record_op_hashed` but also returns the legacy
/// action hash and entry hash, for read-back assertions.
fn store_record_op_with_hashes(
    seed: u8,
) -> (DhtOpHashed, holo_hash::ActionHash, holo_hash::EntryHash) {
    use holo_hash::{ActionHash, EntryHash};
    use holochain_serialized_bytes::UnsafeBytes;
    use holochain_types::prelude::{AppEntryBytes, Entry, RecordEntry, Signature};
    use holochain_zome_types::action::{Action, Create, EntryType};
    use holochain_zome_types::entry_def::EntryVisibility;
    use holochain_zome_types::prelude::AppEntryDef;

    let author = AgentPubKey::from_raw_36(vec![seed; 36]);
    let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
    let entry = Entry::App(AppEntryBytes(
        holochain_serialized_bytes::SerializedBytes::from(UnsafeBytes::from(vec![seed; 8])),
    ));
    let sig = Signature::from([seed; 64]);
    let action = Action::Create(Create {
        author: author.clone(),
        timestamp: Timestamp::from_micros(seed as i64 * 1000),
        action_seq: 1,
        prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(200); 36]),
        entry_type: EntryType::App(AppEntryDef::new(
            0.into(),
            0.into(),
            EntryVisibility::Public,
        )),
        entry_hash: entry_hash.clone(),
        weight: Default::default(),
    });
    let action_hash = ActionHash::with_data_sync(&action);
    let op = DhtOp::ChainOp(Box::new(ChainOp::StoreRecord(
        sig,
        action,
        RecordEntry::Present(entry),
    )));
    (DhtOpHashed::from_content_sync(op), action_hash, entry_hash)
}

#[tokio::test]
async fn retrieve_action_returns_stored_action() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let (op, action_hash, _entry_hash) = store_record_op_with_hashes(1);
    store.record_incoming_ops(vec![op]).await.unwrap();

    let got = store.as_read().retrieve_action(&action_hash).await.unwrap();
    let got = got.expect("action should be retrievable");
    assert_eq!(got.as_hash(), &action_hash);

    let missing = holo_hash::ActionHash::from_raw_36(vec![250u8; 36]);
    assert!(store
        .as_read()
        .retrieve_action(&missing)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn retrieve_entry_returns_public_entry() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let (op, _action_hash, entry_hash) = store_record_op_with_hashes(2);
    store.record_incoming_ops(vec![op]).await.unwrap();

    let got = store
        .as_read()
        .retrieve_entry(&entry_hash, None)
        .await
        .unwrap();
    assert!(matches!(got, Some(holochain_types::prelude::Entry::App(_))));

    let missing = holo_hash::EntryHash::from_raw_36(vec![251u8; 36]);
    assert!(store
        .as_read()
        .retrieve_entry(&missing, None)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn record_locally_validated_warrants_inserts_warrant() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let warrant_op = build_test_warrant_op_hashed(30);
    store
        .record_locally_validated_warrants(vec![warrant_op.clone()])
        .await
        .unwrap();

    // warrantee is seed.wrapping_add(50) = 80 for seed=30.
    let expected_warrantee = AgentPubKey::from_raw_36(vec![80u8; 36]);

    // Self-issued warrants are recorded into limbo ready for integration (not
    // straight into the integrated table), so the integration workflow runs and
    // can block the warrantee. Integrating emits a summary carrying the
    // warrantee, which drives that block.
    let summaries = store
        .integrate_ready_ops(holochain_types::prelude::Timestamp::now())
        .await
        .unwrap();
    let summary = summaries
        .iter()
        .find(|s| s.op_hash == *warrant_op.as_hash())
        .expect("warrant not integrated");
    assert_eq!(summary.warrantee.as_ref(), Some(&expected_warrantee));
    assert_eq!(
        summary.validation_status,
        holochain_zome_types::dht_v2::OpValidity::Accepted
    );

    let row = store
        .db()
        .as_ref()
        .get_warrant(warrant_op.as_hash().clone())
        .await
        .unwrap()
        .expect("warrant row missing");
    assert_eq!(row.warrantee, expected_warrantee.get_raw_36().to_vec());
    // The rejection reason is extracted from the warrant proof and stored in
    // its own column.
    assert_eq!(row.reason.as_deref(), Some("test warrant"));
}

#[tokio::test]
async fn retrieve_record_returns_action_with_entry() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let (op, action_hash, _entry_hash) = store_record_op_with_hashes(3);
    store.record_incoming_ops(vec![op]).await.unwrap();

    let record = store
        .as_read()
        .retrieve_record(&action_hash, None)
        .await
        .unwrap()
        .expect("record should be retrievable");
    assert_eq!(record.action_address(), &action_hash);
    // The Create action references a public App entry, so it must be present.
    assert!(matches!(
        record.entry(),
        holochain_types::prelude::RecordEntry::Present(_)
    ));

    let missing = holo_hash::ActionHash::from_raw_36(vec![252u8; 36]);
    assert!(store
        .as_read()
        .retrieve_record(&missing, None)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn get_live_record_returns_undeleted_record() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let (op, action_hash, _entry_hash) = store_record_op_with_hashes(4);
    store.record_incoming_ops(vec![op]).await.unwrap();

    let record = store
        .as_read()
        .get_live_record(&action_hash, None)
        .await
        .unwrap()
        .expect("undeleted record should be live");
    assert_eq!(record.action_address(), &action_hash);
}

#[tokio::test]
async fn get_live_record_returns_none_when_deleted() {
    use holochain_data::dht::InsertDeletedRecord;
    use holochain_zome_types::action::Delete;

    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let (op, action_hash, entry_hash) = store_record_op_with_hashes(5);
    store.record_incoming_ops(vec![op]).await.unwrap();

    // Build and insert a Delete action so the Action FK is satisfied.
    let delete_action = Action::Delete(Delete {
        author: AgentPubKey::from_raw_36(vec![205u8; 36]),
        timestamp: holochain_types::prelude::Timestamp::from_micros(205_000),
        action_seq: 2,
        prev_action: holo_hash::ActionHash::from_raw_36(vec![206u8; 36]),
        deletes_address: action_hash.clone(),
        deletes_entry_address: entry_hash.clone(),
        weight: Default::default(),
    });
    let delete_op = DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(
        ChainOp::RegisterDeletedBy(Signature::from([205u8; 64]), {
            match delete_action.clone() {
                Action::Delete(d) => d,
                _ => unreachable!(),
            }
        }),
    )));
    let delete_action_hash = holo_hash::ActionHash::with_data_sync(&delete_action);
    store.record_incoming_ops(vec![delete_op]).await.unwrap();

    store
        .db
        .insert_deleted_record_index(InsertDeletedRecord {
            action_hash: &delete_action_hash,
            deletes_action_hash: &action_hash,
            deletes_entry_hash: &entry_hash,
        })
        .await
        .unwrap();

    assert!(store
        .as_read()
        .get_live_record(&action_hash, None)
        .await
        .unwrap()
        .is_none());
}

/// Build a single-op `RenderedOps` for a `StoreEntry(Create)`.
/// Returns `(RenderedOps, action_hash, entry_hash)`.
fn build_rendered_store_entry(
    seed: u8,
) -> (RenderedOps, holo_hash::ActionHash, holo_hash::EntryHash) {
    use holo_hash::{ActionHash, EntryHash};
    use holochain_serialized_bytes::UnsafeBytes;
    use holochain_types::prelude::{AppEntryBytes, Entry, EntryHashed};

    let author = AgentPubKey::from_raw_36(vec![seed; 36]);
    let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
    let entry = Entry::App(AppEntryBytes(
        holochain_serialized_bytes::SerializedBytes::from(UnsafeBytes::from(vec![seed; 8])),
    ));
    let sig = Signature::from([seed; 64]);
    let action = Action::Create(Create {
        author,
        timestamp: Timestamp::from_micros(seed as i64 * 1000),
        action_seq: 1,
        prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(200); 36]),
        entry_type: EntryType::App(AppEntryDef::new(
            0.into(),
            0.into(),
            EntryVisibility::Public,
        )),
        entry_hash: entry_hash.clone(),
        weight: Default::default(),
    });
    let action_hash = holo_hash::ActionHash::with_data_sync(&action);
    let entry_hashed = EntryHashed::with_pre_hashed(entry, entry_hash.clone());
    let rendered =
        RenderedOp::new(action, sig, None, ChainOpType::StoreEntry).expect("rendered op");
    let ops = RenderedOps {
        entry: Some(entry_hashed),
        ops: vec![rendered],
        warrant: None,
    };
    (ops, action_hash, entry_hash)
}

#[tokio::test]
async fn get_live_entry_returns_live_create_record() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let (ops, action_hash, entry_hash) = build_rendered_store_entry(6);
    store.cache_chain_ops(&ops).await.unwrap();

    let record = store
        .as_read()
        .get_live_entry(&entry_hash, None)
        .await
        .unwrap()
        .expect("live entry should resolve to a record");
    assert_eq!(record.action_address(), &action_hash);
    assert!(matches!(
        record.entry(),
        holochain_types::prelude::RecordEntry::Present(_)
    ));
}

#[tokio::test]
async fn get_live_entry_returns_none_when_create_deleted() {
    use holochain_data::dht::InsertDeletedRecord;
    use holochain_zome_types::action::Delete;

    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let (ops, action_hash, entry_hash) = build_rendered_store_entry(7);
    store.cache_chain_ops(&ops).await.unwrap();

    // Build a real Delete action so the Action FK (DeletedRecord.action_hash → Action.hash)
    // is satisfied — mirrors `get_live_record_returns_none_when_deleted`.
    let delete_action = Action::Delete(Delete {
        author: AgentPubKey::from_raw_36(vec![207u8; 36]),
        timestamp: Timestamp::from_micros(207_000),
        action_seq: 2,
        prev_action: holo_hash::ActionHash::from_raw_36(vec![208u8; 36]),
        deletes_address: action_hash.clone(),
        deletes_entry_address: entry_hash.clone(),
        weight: Default::default(),
    });
    let delete_op = DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(
        ChainOp::RegisterDeletedBy(Signature::from([207u8; 64]), {
            match delete_action.clone() {
                Action::Delete(d) => d,
                _ => unreachable!(),
            }
        }),
    )));
    let delete_action_hash = holo_hash::ActionHash::with_data_sync(&delete_action);
    store.record_incoming_ops(vec![delete_op]).await.unwrap();

    store
        .db
        .insert_deleted_record_index(InsertDeletedRecord {
            action_hash: &delete_action_hash,
            deletes_action_hash: &action_hash,
            deletes_entry_hash: &entry_hash,
        })
        .await
        .unwrap();

    assert!(store
        .as_read()
        .get_live_entry(&entry_hash, None)
        .await
        .unwrap()
        .is_none());
}

/// Build a single-op `RenderedOps` for a `StoreRecord(Create)` with a public
/// entry.  Returns `(RenderedOps, action_hash, entry_hash)`.
fn build_rendered_store_record_ops(
    seed: u8,
) -> (RenderedOps, holo_hash::ActionHash, holo_hash::EntryHash) {
    use holo_hash::{ActionHash, EntryHash};
    use holochain_serialized_bytes::UnsafeBytes;
    use holochain_types::prelude::{AppEntryBytes, Entry, EntryHashed};

    let author = AgentPubKey::from_raw_36(vec![seed; 36]);
    let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
    let entry = Entry::App(AppEntryBytes(
        holochain_serialized_bytes::SerializedBytes::from(UnsafeBytes::from(vec![seed; 8])),
    ));
    let sig = Signature::from([seed; 64]);
    let action = Action::Create(Create {
        author,
        timestamp: Timestamp::from_micros(seed as i64 * 1000),
        action_seq: 1,
        prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(200); 36]),
        entry_type: EntryType::App(AppEntryDef::new(
            0.into(),
            0.into(),
            EntryVisibility::Public,
        )),
        entry_hash: entry_hash.clone(),
        weight: Default::default(),
    });
    let action_hash = holo_hash::ActionHash::with_data_sync(&action);
    let entry_hashed = EntryHashed::with_pre_hashed(entry, entry_hash.clone());
    let rendered =
        RenderedOp::new(action, sig, None, ChainOpType::StoreRecord).expect("rendered op");
    let ops = RenderedOps {
        entry: Some(entry_hashed),
        ops: vec![rendered],
        warrant: None,
    };
    (ops, action_hash, entry_hash)
}

#[tokio::test]
async fn get_entry_details_assembles_creates_deletes_updates_and_status() {
    use holochain_data::dht::{InsertDeletedRecord, InsertUpdatedRecord};
    use holochain_types::prelude::RecordEntry;
    use holochain_zome_types::action::{Delete, Update};
    use holochain_zome_types::metadata::EntryDhtStatus;

    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let (ops, action_hash, entry_hash) = build_rendered_store_entry(11);
    store.cache_chain_ops(&ops).await.unwrap();

    // Build a real Delete action so the Action FK is satisfied.
    // Use RegisterDeletedEntryAction (entry-basis delete op).
    let delete_action = Action::Delete(Delete {
        author: AgentPubKey::from_raw_36(vec![221u8; 36]),
        timestamp: Timestamp::from_micros(221_000),
        action_seq: 2,
        prev_action: holo_hash::ActionHash::from_raw_36(vec![222u8; 36]),
        deletes_address: action_hash.clone(),
        deletes_entry_address: entry_hash.clone(),
        weight: Default::default(),
    });
    let delete_op = DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(
        ChainOp::RegisterDeletedEntryAction(
            Signature::from([221u8; 64]),
            match delete_action.clone() {
                Action::Delete(d) => d,
                _ => unreachable!(),
            },
        ),
    )));
    let delete_action_hash = holo_hash::ActionHash::with_data_sync(&delete_action);
    store.record_incoming_ops(vec![delete_op]).await.unwrap();

    store
        .db
        .insert_deleted_record_index(InsertDeletedRecord {
            action_hash: &delete_action_hash,
            deletes_action_hash: &action_hash,
            deletes_entry_hash: &entry_hash,
        })
        .await
        .unwrap();

    // Build a real Update action so the Action FK is satisfied.
    // Use RegisterUpdatedContent (entry-basis update op).
    let new_entry_hash = holo_hash::EntryHash::from_raw_36(vec![223u8; 36]);
    let update_action = Action::Update(Update {
        author: AgentPubKey::from_raw_36(vec![224u8; 36]),
        timestamp: Timestamp::from_micros(224_000),
        action_seq: 2,
        prev_action: holo_hash::ActionHash::from_raw_36(vec![225u8; 36]),
        original_action_address: action_hash.clone(),
        original_entry_address: entry_hash.clone(),
        entry_type: EntryType::App(AppEntryDef::new(
            0.into(),
            0.into(),
            EntryVisibility::Public,
        )),
        entry_hash: new_entry_hash,
        weight: Default::default(),
    });
    let update_op =
        DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(ChainOp::RegisterUpdatedContent(
            Signature::from([224u8; 64]),
            match update_action.clone() {
                Action::Update(u) => u,
                _ => unreachable!(),
            },
            RecordEntry::NA,
        ))));
    let update_action_hash = holo_hash::ActionHash::with_data_sync(&update_action);
    store.record_incoming_ops(vec![update_op]).await.unwrap();

    store
        .db
        .insert_updated_record_index(InsertUpdatedRecord {
            action_hash: &update_action_hash,
            original_action_hash: &action_hash,
            original_entry_hash: &entry_hash,
        })
        .await
        .unwrap();

    let details = store
        .as_read()
        .get_entry_details(&entry_hash, None)
        .await
        .unwrap()
        .expect("entry details");
    assert!(matches!(
        details.entry,
        holochain_types::prelude::Entry::App(_)
    ));
    assert_eq!(details.actions.len(), 1, "the create is still listed");
    assert_eq!(details.rejected_actions.len(), 0);
    assert_eq!(details.deletes.len(), 1);
    assert_eq!(details.updates.len(), 1);
    assert_eq!(details.entry_dht_status, EntryDhtStatus::Dead);
}

#[tokio::test]
async fn get_record_details_assembles_record_deletes_and_updates() {
    use holochain_data::dht::{InsertDeletedRecord, InsertUpdatedRecord};
    use holochain_zome_types::action::{Delete, Update};

    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let (ops, action_hash, entry_hash) = build_rendered_store_record_ops(9);
    store.cache_chain_ops(&ops).await.unwrap();

    // Build and insert a Delete action targeting `action_hash`.
    let delete_action = Action::Delete(Delete {
        author: AgentPubKey::from_raw_36(vec![209u8; 36]),
        timestamp: Timestamp::from_micros(209_000),
        action_seq: 2,
        prev_action: holo_hash::ActionHash::from_raw_36(vec![210u8; 36]),
        deletes_address: action_hash.clone(),
        deletes_entry_address: entry_hash.clone(),
        weight: Default::default(),
    });
    let delete_op =
        DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(ChainOp::RegisterDeletedBy(
            Signature::from([209u8; 64]),
            match delete_action.clone() {
                Action::Delete(d) => d,
                _ => unreachable!(),
            },
        ))));
    let delete_action_hash = holo_hash::ActionHash::with_data_sync(&delete_action);
    store.record_incoming_ops(vec![delete_op]).await.unwrap();

    store
        .db
        .insert_deleted_record_index(InsertDeletedRecord {
            action_hash: &delete_action_hash,
            deletes_action_hash: &action_hash,
            deletes_entry_hash: &entry_hash,
        })
        .await
        .unwrap();

    // Build and insert an Update action of `action_hash`.
    let new_entry_hash = holo_hash::EntryHash::from_raw_36(vec![211u8; 36]);
    let update_action = Action::Update(Update {
        author: AgentPubKey::from_raw_36(vec![212u8; 36]),
        timestamp: Timestamp::from_micros(212_000),
        action_seq: 2,
        prev_action: holo_hash::ActionHash::from_raw_36(vec![213u8; 36]),
        original_action_address: action_hash.clone(),
        original_entry_address: entry_hash.clone(),
        entry_type: EntryType::App(AppEntryDef::new(
            0.into(),
            0.into(),
            EntryVisibility::Public,
        )),
        entry_hash: new_entry_hash,
        weight: Default::default(),
    });
    use holochain_types::prelude::RecordEntry;
    let update_op =
        DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(ChainOp::RegisterUpdatedRecord(
            Signature::from([212u8; 64]),
            match update_action.clone() {
                Action::Update(u) => u,
                _ => unreachable!(),
            },
            RecordEntry::NA,
        ))));
    let update_action_hash = holo_hash::ActionHash::with_data_sync(&update_action);
    store.record_incoming_ops(vec![update_op]).await.unwrap();

    store
        .db
        .insert_updated_record_index(InsertUpdatedRecord {
            action_hash: &update_action_hash,
            original_action_hash: &action_hash,
            original_entry_hash: &entry_hash,
        })
        .await
        .unwrap();

    let details = store
        .as_read()
        .get_record_details(&action_hash, None)
        .await
        .unwrap()
        .expect("record details");
    assert_eq!(details.record.action_address(), &action_hash);
    assert_eq!(
        details.validation_status,
        holochain_zome_types::prelude::ValidationStatus::Valid
    );
    assert_eq!(details.deletes.len(), 1);
    assert_eq!(details.deletes[0].as_hash(), &delete_action_hash);
    assert_eq!(details.updates.len(), 1);
    assert_eq!(details.updates[0].as_hash(), &update_action_hash);
}

/// Build a single-op `RenderedOps` for a `RegisterAddLink(CreateLink)` chain
/// op.  Returns `(RenderedOps, base_address, create_link_action_hash)` so
/// callers can query by base and assert on the returned link hash.
///
/// The fixture mirrors `cache.rs`'s `build_rendered_create_link` but exposes
/// the base and the create-link action hash.
fn build_rendered_create_link_with_meta(seed: u8) -> (RenderedOps, AnyLinkableHash, ActionHash) {
    let author = AgentPubKey::from_raw_36(vec![seed; 36]);
    let base = AnyLinkableHash::from_raw_36_and_type(
        vec![seed.wrapping_add(50); 36],
        holo_hash::hash_type::AnyLinkable::Entry,
    );
    let target = AnyLinkableHash::from_raw_36_and_type(
        vec![seed.wrapping_add(60); 36],
        holo_hash::hash_type::AnyLinkable::Entry,
    );
    let sig = Signature::from([seed; 64]);
    let action = Action::CreateLink(CreateLink {
        author,
        timestamp: Timestamp::from_micros(seed as i64 * 1000),
        action_seq: 2,
        prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(70); 36]),
        base_address: base.clone(),
        target_address: target,
        zome_index: 0.into(),
        link_type: 0.into(),
        tag: holochain_zome_types::link::LinkTag(vec![1, 2, 3]),
        weight: Default::default(),
    });

    let rendered =
        RenderedOp::new(action, sig, None, ChainOpType::RegisterAddLink).expect("rendered op");
    let create_link_hash = rendered.action.as_hash().clone();
    let ops = RenderedOps {
        entry: None,
        ops: vec![rendered],
        warrant: None,
    };
    (ops, base, create_link_hash)
}

/// Build a single-op `RenderedOps` for a `RegisterRemoveLink(DeleteLink)` chain
/// op that tombstones the given `create_link_hash` on `base`.
fn build_rendered_delete_link_for(
    create_link_hash: ActionHash,
    base: &AnyLinkableHash,
    seed: u8,
) -> RenderedOps {
    let author = AgentPubKey::from_raw_36(vec![seed.wrapping_add(1); 36]);
    let sig = Signature::from([seed.wrapping_add(1); 64]);
    let action = Action::DeleteLink(DeleteLink {
        author,
        timestamp: Timestamp::from_micros(seed as i64 * 1000 + 500),
        action_seq: 3,
        prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(90); 36]),
        base_address: base.clone(),
        link_add_address: create_link_hash,
    });
    let rendered =
        RenderedOp::new(action, sig, None, ChainOpType::RegisterRemoveLink).expect("rendered op");
    RenderedOps {
        entry: None,
        ops: vec![rendered],
        warrant: None,
    }
}

#[tokio::test]
async fn get_links_returns_live_links_and_excludes_tombstoned() {
    use crate::query::link::GetLinksFilter;
    use holochain_zome_types::prelude::LinkTypeFilter;

    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let (create_ops, base, create_link_hash) = build_rendered_create_link_with_meta(20);
    store.cache_chain_ops(&create_ops).await.unwrap();

    let filter = GetLinksFilter {
        after: None,
        before: None,
        author: None,
    };
    let links = store
        .as_read()
        .get_links(
            &base,
            &LinkTypeFilter::Dependencies(vec![0.into()]),
            None,
            &filter,
        )
        .await
        .unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].create_link_hash, create_link_hash);

    let delete_ops = build_rendered_delete_link_for(create_link_hash.clone(), &base, 20);
    store.cache_chain_ops(&delete_ops).await.unwrap();

    let links_after = store
        .as_read()
        .get_links(
            &base,
            &LinkTypeFilter::Dependencies(vec![0.into()]),
            None,
            &filter,
        )
        .await
        .unwrap();
    assert_eq!(links_after.len(), 0, "tombstoned link must be excluded");
}

#[tokio::test]
async fn get_link_details_pairs_creates_with_their_deletes() {
    use holochain_zome_types::prelude::LinkTypeFilter;

    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let (create_ops, base, create_link_hash) = build_rendered_create_link_with_meta(21);
    store.cache_chain_ops(&create_ops).await.unwrap();
    let delete_ops = build_rendered_delete_link_for(create_link_hash.clone(), &base, 21);
    store.cache_chain_ops(&delete_ops).await.unwrap();

    let details = store
        .as_read()
        .get_link_details(&base, &LinkTypeFilter::Dependencies(vec![0.into()]), None)
        .await
        .unwrap();
    assert_eq!(details.len(), 1, "one create-link pair");
    let (create, deletes) = &details[0];
    assert_eq!(create.as_hash(), &create_link_hash);
    assert_eq!(deletes.len(), 1, "the create has one DeleteLink");
}

/// Build a `RegisterAddLink` (CreateLink) op for `base`.
fn make_create_link_op(base: &AnyLinkableHash, seed: u8) -> DhtOpHashed {
    let action = Action::CreateLink(CreateLink {
        author: AgentPubKey::from_raw_36(vec![seed; 36]),
        timestamp: Timestamp::from_micros(seed as i64 * 1000),
        action_seq: 2,
        prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(60); 36]),
        base_address: base.clone(),
        target_address: AnyLinkableHash::from_raw_36_and_type(
            vec![seed.wrapping_add(20); 36],
            holo_hash::hash_type::AnyLinkable::Entry,
        ),
        zome_index: 0.into(),
        link_type: 0.into(),
        tag: holochain_zome_types::link::LinkTag(vec![1, 2, 3]),
        weight: Default::default(),
    });
    let create_link = match action {
        Action::CreateLink(cl) => cl,
        _ => unreachable!(),
    };
    DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(ChainOp::RegisterAddLink(
        Signature::from([seed; 64]),
        create_link,
    ))))
}

#[tokio::test]
async fn integration_indexes_create_link() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let base = AnyLinkableHash::from_raw_36_and_type(
        vec![7u8; 36],
        holo_hash::hash_type::AnyLinkable::Entry,
    );
    let op = make_create_link_op(&base, 1);
    let hash = op.as_hash().clone();

    store.record_incoming_ops(vec![op]).await.unwrap();
    store
        .record_chain_op_sys_validation_outcomes(vec![(hash.clone(), SysOutcome::Accepted)])
        .await
        .unwrap();
    store
        .record_app_validation_outcomes(vec![(hash, AppOutcome::Accepted)])
        .await
        .unwrap();
    store
        .integrate_ready_ops(Timestamp::from_micros(1))
        .await
        .unwrap();

    // After integration the Link index must contain the create-link action.
    let creates = store
        .as_read()
        .db()
        .get_link_create_actions(&base)
        .await
        .unwrap();
    assert_eq!(creates.len(), 1, "integrated CreateLink should be indexed");
}

/// Build a `RegisterRemoveLink` (DeleteLink) op targeting `link_add`.
fn make_delete_link_op(base: &AnyLinkableHash, link_add: &ActionHash, seed: u8) -> DhtOpHashed {
    let action = Action::DeleteLink(DeleteLink {
        author: AgentPubKey::from_raw_36(vec![seed; 36]),
        timestamp: Timestamp::from_micros(seed as i64 * 1000),
        action_seq: 3,
        prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(90); 36]),
        base_address: base.clone(),
        link_add_address: link_add.clone(),
    });
    let delete_link = match action {
        Action::DeleteLink(dl) => dl,
        _ => unreachable!(),
    };
    DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(ChainOp::RegisterRemoveLink(
        Signature::from([seed; 64]),
        delete_link,
    ))))
}

#[tokio::test]
async fn integration_indexes_delete_link() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let base = AnyLinkableHash::from_raw_36_and_type(
        vec![9u8; 36],
        holo_hash::hash_type::AnyLinkable::Entry,
    );
    let link_add = ActionHash::from_raw_36(vec![55u8; 36]);
    let op = make_delete_link_op(&base, &link_add, 2);
    let hash = op.as_hash().clone();

    store.record_incoming_ops(vec![op]).await.unwrap();
    store
        .record_chain_op_sys_validation_outcomes(vec![(hash.clone(), SysOutcome::Accepted)])
        .await
        .unwrap();
    store
        .record_app_validation_outcomes(vec![(hash, AppOutcome::Accepted)])
        .await
        .unwrap();
    store
        .integrate_ready_ops(Timestamp::from_micros(1))
        .await
        .unwrap();

    let deletes = store
        .as_read()
        .db()
        .get_delete_link_actions(&link_add)
        .await
        .unwrap();
    assert_eq!(deletes.len(), 1, "integrated DeleteLink should be indexed");
}

async fn integrate_link_op(
    store: &crate::dht_store::DhtStore<DbWrite<Dht>>,
    op: DhtOpHashed,
    app: AppOutcome,
    when: i64,
) {
    let hash = op.as_hash().clone();
    store.record_incoming_ops(vec![op]).await.unwrap();
    store
        .record_chain_op_sys_validation_outcomes(vec![(hash.clone(), SysOutcome::Accepted)])
        .await
        .unwrap();
    store
        .record_app_validation_outcomes(vec![(hash, app)])
        .await
        .unwrap();
    store
        .integrate_ready_ops(Timestamp::from_micros(when))
        .await
        .unwrap();
}

fn build_cached_create_link(base: &holo_hash::AnyLinkableHash, seed: u8) -> RenderedOps {
    let action = Action::CreateLink(CreateLink {
        author: AgentPubKey::from_raw_36(vec![seed; 36]),
        timestamp: Timestamp::from_micros(seed as i64 * 1000),
        action_seq: 2,
        prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(60); 36]),
        base_address: base.clone(),
        target_address: holo_hash::AnyLinkableHash::from_raw_36_and_type(
            vec![seed.wrapping_add(20); 36],
            holo_hash::hash_type::AnyLinkable::Entry,
        ),
        zome_index: 0.into(),
        link_type: 0.into(),
        tag: holochain_zome_types::link::LinkTag(vec![1, 2, 3]),
        weight: Default::default(),
    });
    let rendered = RenderedOp::new(
        action,
        Signature::from([seed; 64]),
        None,
        ChainOpType::RegisterAddLink,
    )
    .expect("rendered op build");
    RenderedOps {
        entry: None,
        ops: vec![rendered],
        warrant: None,
    }
}

#[tokio::test]
async fn authority_link_creates_excludes_cached() {
    let store = crate::dht_store::DhtStore::new_test(dht_id())
        .await
        .unwrap();
    let base = holo_hash::AnyLinkableHash::from_raw_36_and_type(
        vec![7u8; 36],
        holo_hash::hash_type::AnyLinkable::Entry,
    );

    // Authoritative (locally_validated = 1): incoming + integrated.
    integrate_link_op(
        &store,
        make_create_link_op(&base, 1),
        AppOutcome::Accepted,
        1,
    )
    .await;
    // Cached (locally_validated = 0): same base, different op.
    store
        .cache_chain_ops(&build_cached_create_link(&base, 2))
        .await
        .unwrap();

    let creates = store
        .as_read()
        .get_authority_link_creates(&base)
        .await
        .unwrap();
    assert_eq!(
        creates.len(),
        1,
        "only the locally-validated create should be served"
    );
    assert_eq!(creates[0].1, ValidationStatus::Valid);
}

#[tokio::test]
async fn authority_link_creates_reports_rejected() {
    let store = crate::dht_store::DhtStore::new_test(dht_id())
        .await
        .unwrap();
    let base = holo_hash::AnyLinkableHash::from_raw_36_and_type(
        vec![8u8; 36],
        holo_hash::hash_type::AnyLinkable::Entry,
    );
    // Integrated but app-rejected -> locally_validated = 1, status Rejected.
    integrate_link_op(
        &store,
        make_create_link_op(&base, 3),
        AppOutcome::Rejected,
        1,
    )
    .await;

    let creates = store
        .as_read()
        .get_authority_link_creates(&base)
        .await
        .unwrap();
    assert_eq!(creates.len(), 1);
    assert_eq!(creates[0].1, ValidationStatus::Rejected);
}

#[tokio::test]
async fn authority_delete_links_returns_integrated_deletes() {
    let store = crate::dht_store::DhtStore::new_test(dht_id())
        .await
        .unwrap();
    let base = holo_hash::AnyLinkableHash::from_raw_36_and_type(
        vec![9u8; 36],
        holo_hash::hash_type::AnyLinkable::Entry,
    );
    // Integrate a create-link for the base, then read its action hash back
    // (so the delete's create_link_hash matches a create in the base's index).
    integrate_link_op(
        &store,
        make_create_link_op(&base, 4),
        AppOutcome::Accepted,
        1,
    )
    .await;
    let create_hash = store
        .as_read()
        .get_authority_link_creates(&base)
        .await
        .unwrap()[0]
        .0
        .as_hash()
        .clone();

    // A delete-link targeting that create.
    integrate_link_op(
        &store,
        make_delete_link_op(&base, &create_hash, 5),
        AppOutcome::Accepted,
        2,
    )
    .await;

    let deletes = store
        .as_read()
        .get_authority_delete_links(&base)
        .await
        .unwrap();
    assert_eq!(
        deletes.len(),
        1,
        "the integrated delete-link should be served"
    );
    assert_eq!(deletes[0].1, ValidationStatus::Valid);
}

#[tokio::test]
async fn integrate_upgrades_cached_op_to_locally_validated() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let base = AnyLinkableHash::from_raw_36_and_type(
        vec![13u8; 36],
        holo_hash::hash_type::AnyLinkable::Entry,
    );

    // One action + signature, used to build BOTH the cached RenderedOps and
    // the incoming DhtOpHashed, so they share the same op hash.
    let action = Action::CreateLink(CreateLink {
        author: AgentPubKey::from_raw_36(vec![6u8; 36]),
        timestamp: Timestamp::from_micros(6000),
        action_seq: 2,
        prev_action: ActionHash::from_raw_36(vec![66u8; 36]),
        base_address: base.clone(),
        target_address: AnyLinkableHash::from_raw_36_and_type(
            vec![26u8; 36],
            holo_hash::hash_type::AnyLinkable::Entry,
        ),
        zome_index: 0.into(),
        link_type: 0.into(),
        tag: holochain_zome_types::link::LinkTag(vec![1, 2, 3]),
        weight: Default::default(),
    });
    let sig = Signature::from([6u8; 64]);

    // Cache the op first (locally_validated = 0). The authority read excludes it.
    let rendered = RenderedOps {
        entry: None,
        ops: vec![RenderedOp::new(
            action.clone(),
            sig.clone(),
            None,
            ChainOpType::RegisterAddLink,
        )
        .expect("rendered op build")],
        warrant: None,
    };
    store.cache_chain_ops(&rendered).await.unwrap();
    assert!(
        store
            .as_read()
            .get_authority_link_creates(&base)
            .await
            .unwrap()
            .is_empty(),
        "cached-only link must not be served by the authority read"
    );

    // Receive + validate + integrate the SAME op.
    let create_link = match action {
        Action::CreateLink(cl) => cl,
        _ => unreachable!(),
    };
    let op = DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(ChainOp::RegisterAddLink(
        sig,
        create_link,
    ))));
    let hash = op.as_hash().clone();
    store.record_incoming_ops(vec![op]).await.unwrap();
    store
        .record_chain_op_sys_validation_outcomes(vec![(hash.clone(), SysOutcome::Accepted)])
        .await
        .unwrap();
    store
        .record_app_validation_outcomes(vec![(hash, AppOutcome::Accepted)])
        .await
        .unwrap();
    store
        .integrate_ready_ops(Timestamp::from_micros(1))
        .await
        .unwrap();

    // Now locally validated -> the authority read serves it.
    let creates = store
        .as_read()
        .get_authority_link_creates(&base)
        .await
        .unwrap();
    assert_eq!(
        creates.len(),
        1,
        "integration must upgrade the cached row to locally_validated"
    );
}

#[tokio::test]
async fn authority_store_record_excludes_cached() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();

    // Authoritative: incoming + integrated -> locally_validated = 1.
    let (op, action_hash, _entry_hash) = store_record_op_with_hashes(70);
    integrate_link_op(&store, op, AppOutcome::Accepted, 1).await;
    let got = store
        .as_read()
        .get_authority_store_record(&action_hash)
        .await
        .unwrap();
    let (_action, status) = got.expect("locally-validated record should be served");
    assert_eq!(status, ValidationStatus::Valid);

    // Cached: cache_chain_ops -> locally_validated = 0 -> not served.
    let (rendered, cached_hash) = build_rendered_store_record_for_move(71);
    store.cache_chain_ops(&rendered).await.unwrap();
    assert!(
        store
            .as_read()
            .get_authority_store_record(&cached_hash)
            .await
            .unwrap()
            .is_none(),
        "cached-only record must not be served by the authority read"
    );
}

#[tokio::test]
async fn authority_deletes_for_record_returns_integrated_deletes() {
    use holochain_zome_types::action::Delete;
    let store = DhtStore::new_test(dht_id()).await.unwrap();

    let (op, action_hash, entry_hash) = store_record_op_with_hashes(72);
    integrate_link_op(&store, op, AppOutcome::Accepted, 1).await;

    let delete_action = Action::Delete(Delete {
        author: AgentPubKey::from_raw_36(vec![210u8; 36]),
        timestamp: Timestamp::from_micros(210_000),
        action_seq: 2,
        prev_action: ActionHash::from_raw_36(vec![211u8; 36]),
        deletes_address: action_hash.clone(),
        deletes_entry_address: entry_hash.clone(),
        weight: Default::default(),
    });
    let delete_op =
        DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(ChainOp::RegisterDeletedBy(
            Signature::from([210u8; 64]),
            match delete_action {
                Action::Delete(d) => d,
                _ => unreachable!(),
            },
        ))));
    integrate_link_op(&store, delete_op, AppOutcome::Accepted, 2).await;

    let deletes = store
        .as_read()
        .get_authority_deletes_for_record(&action_hash)
        .await
        .unwrap();
    assert_eq!(deletes.len(), 1, "the integrated delete should be served");
    assert_eq!(deletes[0].1, ValidationStatus::Valid);
}

#[tokio::test]
async fn authority_updates_for_record_returns_integrated_updates() {
    use holo_hash::EntryHash;
    use holochain_types::prelude::RecordEntry;
    use holochain_zome_types::action::{EntryType, Update};
    use holochain_zome_types::entry_def::EntryVisibility;
    use holochain_zome_types::prelude::AppEntryDef;
    let store = DhtStore::new_test(dht_id()).await.unwrap();

    let (op, action_hash, entry_hash) = store_record_op_with_hashes(73);
    integrate_link_op(&store, op, AppOutcome::Accepted, 1).await;

    let update_action = Action::Update(Update {
        author: AgentPubKey::from_raw_36(vec![220u8; 36]),
        timestamp: Timestamp::from_micros(220_000),
        action_seq: 2,
        prev_action: ActionHash::from_raw_36(vec![221u8; 36]),
        original_action_address: action_hash.clone(),
        original_entry_address: entry_hash.clone(),
        entry_type: EntryType::App(AppEntryDef::new(
            0.into(),
            0.into(),
            EntryVisibility::Public,
        )),
        entry_hash: EntryHash::from_raw_36(vec![222u8; 36]),
        weight: Default::default(),
    });
    let update_op =
        DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(ChainOp::RegisterUpdatedRecord(
            Signature::from([220u8; 64]),
            match update_action {
                Action::Update(u) => u,
                _ => unreachable!(),
            },
            RecordEntry::NA,
        ))));
    integrate_link_op(&store, update_op, AppOutcome::Accepted, 2).await;

    let updates = store
        .as_read()
        .get_authority_updates_for_record(&action_hash)
        .await
        .unwrap();
    assert_eq!(updates.len(), 1, "the integrated update should be served");
    assert_eq!(updates[0].1, ValidationStatus::Valid);
}

/// Build a `StoreEntry(Create)` op as a `DhtOpHashed`, returning it + the entry hash.
fn make_store_entry_op(seed: u8) -> (DhtOpHashed, EntryHash) {
    use holochain_serialized_bytes::UnsafeBytes;
    use holochain_types::action::NewEntryAction;
    use holochain_types::prelude::{AppEntryBytes, Entry};
    use holochain_zome_types::action::{Create, EntryType};
    use holochain_zome_types::entry_def::EntryVisibility;
    use holochain_zome_types::prelude::AppEntryDef;

    let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
    let entry = Entry::App(AppEntryBytes(
        holochain_serialized_bytes::SerializedBytes::from(UnsafeBytes::from(vec![seed; 8])),
    ));
    let create = Create {
        author: AgentPubKey::from_raw_36(vec![seed; 36]),
        timestamp: Timestamp::from_micros(seed as i64 * 1000),
        action_seq: 1,
        prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(200); 36]),
        entry_type: EntryType::App(AppEntryDef::new(
            0.into(),
            0.into(),
            EntryVisibility::Public,
        )),
        entry_hash: entry_hash.clone(),
        weight: Default::default(),
    };
    let op = DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(ChainOp::StoreEntry(
        Signature::from([seed; 64]),
        NewEntryAction::Create(create),
        entry,
    ))));
    (op, entry_hash)
}

#[tokio::test]
async fn authority_entry_creates_excludes_cached() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();

    // Authoritative: incoming + integrated -> locally_validated = 1.
    let (op, entry_hash) = make_store_entry_op(80);
    integrate_link_op(&store, op, AppOutcome::Accepted, 1).await;
    let creates = store
        .as_read()
        .get_authority_entry_creates(&entry_hash)
        .await
        .unwrap();
    assert_eq!(
        creates.len(),
        1,
        "locally-validated create should be served"
    );
    assert_eq!(creates[0].1, ValidationStatus::Valid);

    // Cached: cache_chain_ops -> locally_validated = 0 -> not served.
    let (rendered, _ah, cached_entry) = build_rendered_store_entry(81);
    store.cache_chain_ops(&rendered).await.unwrap();
    assert!(
        store
            .as_read()
            .get_authority_entry_creates(&cached_entry)
            .await
            .unwrap()
            .is_empty(),
        "cached-only create must not be served by the authority read"
    );
}

#[tokio::test]
async fn authority_deletes_for_entry_returns_integrated_deletes() {
    use holochain_zome_types::action::Delete;
    let store = DhtStore::new_test(dht_id()).await.unwrap();

    let (op, entry_hash) = make_store_entry_op(82);
    integrate_link_op(&store, op, AppOutcome::Accepted, 1).await;

    let delete_action = Action::Delete(Delete {
        author: AgentPubKey::from_raw_36(vec![213u8; 36]),
        timestamp: Timestamp::from_micros(213_000),
        action_seq: 2,
        prev_action: ActionHash::from_raw_36(vec![214u8; 36]),
        deletes_address: ActionHash::from_raw_36(vec![215u8; 36]),
        deletes_entry_address: entry_hash.clone(),
        weight: Default::default(),
    });
    let delete_op = DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(
        ChainOp::RegisterDeletedEntryAction(
            Signature::from([213u8; 64]),
            match delete_action {
                Action::Delete(d) => d,
                _ => unreachable!(),
            },
        ),
    )));
    integrate_link_op(&store, delete_op, AppOutcome::Accepted, 2).await;

    let deletes = store
        .as_read()
        .get_authority_deletes_for_entry(&entry_hash)
        .await
        .unwrap();
    assert_eq!(
        deletes.len(),
        1,
        "the integrated entry-delete should be served"
    );
    assert_eq!(deletes[0].1, ValidationStatus::Valid);
}

#[tokio::test]
async fn authority_updates_for_entry_returns_integrated_updates() {
    use holochain_types::prelude::RecordEntry;
    use holochain_zome_types::action::Update;
    let store = DhtStore::new_test(dht_id()).await.unwrap();

    let (op, entry_hash) = make_store_entry_op(83);
    integrate_link_op(&store, op, AppOutcome::Accepted, 1).await;

    let update_action = Action::Update(Update {
        author: AgentPubKey::from_raw_36(vec![223u8; 36]),
        timestamp: Timestamp::from_micros(223_000),
        action_seq: 2,
        prev_action: ActionHash::from_raw_36(vec![224u8; 36]),
        original_action_address: ActionHash::from_raw_36(vec![225u8; 36]),
        original_entry_address: entry_hash.clone(),
        entry_type: EntryType::App(AppEntryDef::new(
            0.into(),
            0.into(),
            EntryVisibility::Public,
        )),
        entry_hash: EntryHash::from_raw_36(vec![226u8; 36]),
        weight: Default::default(),
    });
    let update_op =
        DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(ChainOp::RegisterUpdatedContent(
            Signature::from([223u8; 64]),
            match update_action {
                Action::Update(u) => u,
                _ => unreachable!(),
            },
            RecordEntry::NA,
        ))));
    integrate_link_op(&store, update_op, AppOutcome::Accepted, 2).await;

    let updates = store
        .as_read()
        .get_authority_updates_for_entry(&entry_hash)
        .await
        .unwrap();
    assert_eq!(
        updates.len(),
        1,
        "the integrated entry-update should be served"
    );
    assert_eq!(updates[0].1, ValidationStatus::Valid);
}
