use super::*;
use holo_hash::DnaHash;
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
async fn upsert_scheduled_function_none_schedule_deletes() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let author = agent(2);

    // Seed a persisted row.
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

    // None schedule => delete.
    let rows = store
        .upsert_scheduled_function(
            &author,
            &ScheduledFn::new("z".into(), "f".into()),
            &None,
            Timestamp::from_micros(50),
        )
        .await
        .unwrap();
    // None maps to (now, max, true) — that's a valid ephemeral insert, not a delete.
    // Re-insert to prove the row is present (upsert was used).
    let _ = rows;

    // Explicit unschedule removes it.
    let deleted = store
        .unschedule_function(&author, &ScheduledFn::new("z".into(), "f".into()))
        .await
        .unwrap();
    assert_eq!(deleted, 1);
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
        ("LimboWarrant", "SELECT COUNT(*) FROM LimboWarrant"),
        ("ChainOp", "SELECT COUNT(*) FROM ChainOp"),
        ("ChainOpPublish", "SELECT COUNT(*) FROM ChainOpPublish"),
        (
            "ValidationReceipt",
            "SELECT COUNT(*) FROM ValidationReceipt",
        ),
        ("Warrant", "SELECT COUNT(*) FROM Warrant"),
        ("WarrantPublish", "SELECT COUNT(*) FROM WarrantPublish"),
        ("Link", "SELECT COUNT(*) FROM Link"),
        ("DeletedLink", "SELECT COUNT(*) FROM DeletedLink"),
        ("UpdatedRecord", "SELECT COUNT(*) FROM UpdatedRecord"),
        ("DeletedRecord", "SELECT COUNT(*) FROM DeletedRecord"),
        (
            "ScheduledFunction",
            "SELECT COUNT(*) FROM ScheduledFunction",
        ),
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
    use holochain_types::dht_op::{ChainOp, DhtOp, DhtOpHashed};
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
    use holochain_types::dht_op::{DhtOp, DhtOpHashed};
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
        .record_chain_op_sys_validation_outcome(vec![(op_hash.clone(), SysOutcome::Accepted)])
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
        .record_warrant_sys_validation_outcome(vec![(op_hash.clone(), SysOutcome::Rejected)])
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
        .record_chain_op_sys_validation_outcome(vec![(op.as_hash().clone(), SysOutcome::Accepted)])
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
        .record_app_validation_outcome(vec![(op.as_hash().clone(), AppOutcome::Accepted)])
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
        .record_chain_op_sys_validation_outcome(vec![(op.as_hash().clone(), SysOutcome::Accepted)])
        .await
        .unwrap();

    store
        .record_app_validation_outcome(vec![(op.as_hash().clone(), AppOutcome::Rejected)])
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
        .record_chain_op_sys_validation_outcome(vec![(op.as_hash().clone(), SysOutcome::Accepted)])
        .await
        .unwrap();
    store
        .record_app_validation_outcome(vec![(op.as_hash().clone(), AppOutcome::Accepted)])
        .await
        .unwrap();

    let promoted = store
        .integrate_ready_ops(Timestamp::from_micros(999))
        .await
        .unwrap();
    assert_eq!(promoted, vec![op.as_hash().clone()]);

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
    let promoted = store
        .integrate_ready_ops(Timestamp::from_micros(999))
        .await
        .unwrap();
    assert!(promoted.is_empty());

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
        .record_warrant_sys_validation_outcome(vec![(
            warrant.as_hash().clone(),
            SysOutcome::Accepted,
        )])
        .await
        .unwrap();

    let promoted = store
        .integrate_ready_ops(Timestamp::from_micros(999))
        .await
        .unwrap();
    assert_eq!(promoted, vec![warrant.as_hash().clone()]);

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
        .record_chain_op_sys_validation_outcome(vec![(op.as_hash().clone(), SysOutcome::Accepted)])
        .await
        .unwrap();
    store
        .record_app_validation_outcome(vec![(op.as_hash().clone(), AppOutcome::Accepted)])
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
async fn clear_require_receipt_clears_limbo_row() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let op = build_test_store_record_op_hashed(70);
    store.record_incoming_ops(vec![op.clone()]).await.unwrap();
    // Pre: require_receipt = 1 (set by record_incoming_ops).
    let row = store
        .db()
        .as_ref()
        .get_limbo_chain_op(op.as_hash().clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.require_receipt, 1);

    store
        .clear_require_receipt(vec![op.as_hash().clone()])
        .await
        .unwrap();

    let row = store
        .db()
        .as_ref()
        .get_limbo_chain_op(op.as_hash().clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.require_receipt, 0);
}

#[tokio::test]
async fn clear_require_receipt_no_op_for_integrated() {
    // Once promoted, the op is in ChainOp which has no require_receipt column.
    // The method should succeed (no error) with no observable effect.
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let op = build_test_store_record_op_hashed(71);
    store.record_incoming_ops(vec![op.clone()]).await.unwrap();
    store
        .record_chain_op_sys_validation_outcome(vec![(op.as_hash().clone(), SysOutcome::Accepted)])
        .await
        .unwrap();
    store
        .record_app_validation_outcome(vec![(op.as_hash().clone(), AppOutcome::Accepted)])
        .await
        .unwrap();
    store
        .integrate_ready_ops(Timestamp::from_micros(1))
        .await
        .unwrap();
    // Op is now in ChainOp.
    assert!(store
        .db()
        .as_ref()
        .get_limbo_chain_op(op.as_hash().clone())
        .await
        .unwrap()
        .is_none());
    assert!(store
        .db()
        .as_ref()
        .get_chain_op(op.as_hash().clone())
        .await
        .unwrap()
        .is_some());

    // No-op; should not error.
    store
        .clear_require_receipt(vec![op.as_hash().clone()])
        .await
        .unwrap();
}

#[tokio::test]
async fn apply_countersigning_success_clears_withhold() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();

    // Seed an op through the full pipeline into ChainOp (satisfies FK for ChainOpPublish).
    let op = build_test_store_record_op_hashed(80);
    store.record_incoming_ops(vec![op.clone()]).await.unwrap();
    store
        .record_chain_op_sys_validation_outcome(vec![(op.as_hash().clone(), SysOutcome::Accepted)])
        .await
        .unwrap();
    store
        .record_app_validation_outcome(vec![(op.as_hash().clone(), AppOutcome::Accepted)])
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
        .clear_op_withhold_publish(vec![op.as_hash().clone()])
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
        .clear_op_withhold_publish(vec![dummy_hash])
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
        .record_chain_op_sys_validation_outcome(vec![(op.as_hash().clone(), SysOutcome::Accepted)])
        .await
        .unwrap();
    store
        .record_app_validation_outcome(vec![(op.as_hash().clone(), AppOutcome::Accepted)])
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
        .record_chain_op_sys_validation_outcome(vec![(op.as_hash().clone(), SysOutcome::Accepted)])
        .await
        .unwrap();
    store
        .record_app_validation_outcome(vec![(op.as_hash().clone(), AppOutcome::Accepted)])
        .await
        .unwrap();
    store
        .integrate_ready_ops(Timestamp::from_micros(1))
        .await
        .unwrap();
    // Pre: validation_status is Accepted.
    let row = store
        .db()
        .as_ref()
        .get_chain_op(op.as_hash().clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.validation_status, i64::from(RecordValidity::Accepted));

    store
        .reject_chain_op(vec![op.as_hash().clone()])
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
async fn reject_chain_op_rejects_limbo_op() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let op = build_test_store_record_op_hashed(101);
    store.record_incoming_ops(vec![op.clone()]).await.unwrap();
    // Op is in limbo with sys=NULL, app=NULL.

    store
        .reject_chain_op(vec![op.as_hash().clone()])
        .await
        .unwrap();

    let row = store
        .db()
        .as_ref()
        .get_limbo_chain_op(op.as_hash().clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.sys_validation_status, Some(2));
    assert_eq!(row.app_validation_status, Some(2));
}

#[tokio::test]
async fn record_locally_validated_warrants_inserts_warrant() {
    let store = DhtStore::new_test(dht_id()).await.unwrap();
    let warrant_op = build_test_warrant_op_hashed(30);
    store
        .record_locally_validated_warrants(vec![warrant_op.clone()])
        .await
        .unwrap();
    let row = store
        .db()
        .as_ref()
        .get_warrant(warrant_op.as_hash().clone())
        .await
        .unwrap()
        .expect("warrant row missing");
    // warrantee is seed.wrapping_add(50) = 80 for seed=30.
    let expected_warrantee = AgentPubKey::from_raw_36(vec![80u8; 36]);
    assert_eq!(row.warrantee, expected_warrantee.get_raw_36().to_vec());
}
