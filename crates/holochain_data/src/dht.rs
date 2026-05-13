//! DHT database operations.
//!
//! Internally split into three layers:
//!
//! - `inner`: free-standing `async fn`s over `sqlx::Executor` per DHT
//!   table (one submodule per table).
//! - `db_operations`: thin `DbRead<Dht>` / `DbWrite<Dht>` method wrappers
//!   that acquire a pool executor and delegate into `inner`.
//! - `tx_operations`: thin `TxRead<Dht>` / `TxWrite<Dht>` method wrappers
//!   that delegate into `inner` using the in-flight transaction.
//!
//! Public API is exposed only via methods on the four handle types
//! ([`crate::DbRead`] / [`crate::DbWrite`] / [`crate::TxRead`] /
//! [`crate::TxWrite`]) plus the parameter structs re-exported here.

mod db_operations;
mod inner;
mod tx_operations;

pub use inner::chain_op::InsertChainOp;
pub use inner::deleted_link::InsertDeletedLink;
pub use inner::deleted_record::InsertDeletedRecord;
pub use inner::limbo_chain_op::InsertLimboChainOp;
pub use inner::limbo_warrant::InsertLimboWarrant;
pub use inner::link::InsertLink;
pub use inner::scheduled_function::InsertScheduledFunction;
pub use inner::updated_record::InsertUpdatedRecord;
pub use inner::warrant::InsertWarrant;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::Dht;
    use crate::test_open_db;
    use holo_hash::{
        ActionHash, AgentPubKey, AnyDhtHash, AnyLinkableHash, DhtOpHash, DnaHash, EntryHash,
        HoloHashed,
    };
    use holochain_integrity_types::dht_v2::{
        Action, ActionData, ActionHeader, DnaData, InitZomesCompleteData, RecordValidity,
    };
    use holochain_integrity_types::entry::Entry;
    use holochain_integrity_types::record::SignedHashed;
    use holochain_integrity_types::signature::Signature;
    use holochain_timestamp::Timestamp;
    use holochain_zome_types::dht_v2::SignedActionHashed;
    use std::sync::Arc;

    fn dht_db_id() -> Dht {
        Dht::new(Arc::new(DnaHash::from_raw_36(vec![0u8; 36])))
    }

    fn sample_action(seed: u8) -> SignedActionHashed {
        let action = Action {
            header: ActionHeader {
                author: AgentPubKey::from_raw_36(vec![1u8; 36]),
                timestamp: Timestamp::from_micros(1_000_000 + seed as i64),
                action_seq: seed as u32,
                prev_action: if seed == 0 {
                    None
                } else {
                    Some(ActionHash::from_raw_36(vec![seed - 1; 36]))
                },
            },
            data: if seed == 0 {
                ActionData::Dna(DnaData {
                    dna_hash: DnaHash::from_raw_36(vec![0u8; 36]),
                })
            } else {
                ActionData::InitZomesComplete(InitZomesCompleteData {})
            },
        };
        let hashed = HoloHashed::with_pre_hashed(action, ActionHash::from_raw_36(vec![seed; 36]));
        SignedHashed::with_presigned(hashed, Signature([seed; 64]))
    }

    #[tokio::test]
    async fn action_roundtrip() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let action = sample_action(0);

        db.insert_action(&action, Some(RecordValidity::Accepted))
            .await
            .unwrap();

        let fetched = db
            .as_ref()
            .get_action(action.as_hash().clone())
            .await
            .unwrap()
            .expect("action not found");

        assert_eq!(fetched, action);
    }

    #[tokio::test]
    async fn actions_by_author() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let inserted: Vec<_> = (0..3u8).map(sample_action).collect();
        for action in &inserted {
            db.insert_action(action, Some(RecordValidity::Accepted))
                .await
                .unwrap();
        }

        let author = AgentPubKey::from_raw_36(vec![1u8; 36]);
        let actions = db.as_ref().get_actions_by_author(author).await.unwrap();
        assert_eq!(actions, inserted);
    }

    #[tokio::test]
    async fn actions_by_author_excludes_other_authors() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let author_a = AgentPubKey::from_raw_36(vec![1u8; 36]);
        let a_inserted: Vec<_> = (0..2u8).map(sample_action).collect();
        for action in &a_inserted {
            db.insert_action(action, None).await.unwrap();
        }

        let other_author = AgentPubKey::from_raw_36(vec![0x99; 36]);
        let other = SignedHashed::with_presigned(
            HoloHashed::with_pre_hashed(
                Action {
                    header: ActionHeader {
                        author: other_author.clone(),
                        timestamp: Timestamp::from_micros(2_000_000),
                        action_seq: 0,
                        prev_action: None,
                    },
                    data: ActionData::Dna(DnaData {
                        dna_hash: DnaHash::from_raw_36(vec![0u8; 36]),
                    }),
                },
                ActionHash::from_raw_36(vec![0xCC; 36]),
            ),
            Signature([0xCC; 64]),
        );
        db.insert_action(&other, None).await.unwrap();

        let a_results = db.as_ref().get_actions_by_author(author_a).await.unwrap();
        assert_eq!(a_results, a_inserted);

        let b_results = db
            .as_ref()
            .get_actions_by_author(other_author)
            .await
            .unwrap();
        assert_eq!(b_results, vec![other]);
    }

    fn sample_entry(seed: u8) -> (EntryHash, Entry) {
        let entry = Entry::App(holochain_integrity_types::entry::AppEntryBytes(
            holochain_serialized_bytes::UnsafeBytes::from(vec![seed; 16]).into(),
        ));
        let hash = EntryHash::from_raw_36(vec![seed; 36]);
        (hash, entry)
    }

    #[tokio::test]
    async fn entry_roundtrip() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let (hash, entry) = sample_entry(7);
        db.insert_entry(&hash, &entry).await.unwrap();
        let fetched = db.as_ref().get_entry(hash.clone(), None).await.unwrap();
        assert_eq!(fetched, Some(entry));
    }

    #[tokio::test]
    async fn private_entry_roundtrip() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let (hash, entry) = sample_entry(11);
        let author = AgentPubKey::from_raw_36(vec![2u8; 36]);
        db.insert_private_entry(&hash, &author, &entry)
            .await
            .unwrap();
        let fetched = db
            .as_ref()
            .get_entry(hash.clone(), Some(&author))
            .await
            .unwrap();
        assert_eq!(fetched, Some(entry));
    }

    #[tokio::test]
    async fn private_entry_isolated_from_entry() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let (hash, entry) = sample_entry(13);
        let author = AgentPubKey::from_raw_36(vec![3u8; 36]);
        db.insert_private_entry(&hash, &author, &entry)
            .await
            .unwrap();
        // Not visible via the public Entry read (no author context).
        assert_eq!(
            db.as_ref().get_entry(hash.clone(), None).await.unwrap(),
            None
        );
        // Not visible to a different author.
        let other = AgentPubKey::from_raw_36(vec![4u8; 36]);
        assert_eq!(
            db.as_ref()
                .get_entry(hash.clone(), Some(&other))
                .await
                .unwrap(),
            None
        );
        // Visible to the owning author.
        assert_eq!(
            db.as_ref()
                .get_entry(hash.clone(), Some(&author))
                .await
                .unwrap(),
            Some(entry)
        );
    }

    /// Verifies that a TxWrite bundling an Action + Entry insert can be rolled back
    /// and neither survives. Also exercises the Tx* wrapper methods.
    #[tokio::test]
    async fn tx_action_and_entry_rollback_discards() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let action = sample_action(0);
        let (entry_hash, entry) = sample_entry(42);

        let mut tx = db.begin().await.unwrap();
        tx.insert_action(&action, Some(RecordValidity::Accepted))
            .await
            .unwrap();
        tx.insert_entry(&entry_hash, &entry).await.unwrap();
        tx.rollback().await.unwrap();

        assert!(db
            .as_ref()
            .get_action(action.as_hash().clone())
            .await
            .unwrap()
            .is_none());
        assert!(db
            .as_ref()
            .get_entry(entry_hash, None)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn cap_grant_roundtrip() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        // Seed the parent Action (FK).
        let action = sample_action(0);
        db.insert_action(&action, Some(RecordValidity::Accepted))
            .await
            .unwrap();

        let author = action.hashed.content.header.author.clone();
        let action_hash = action.as_hash().clone();
        db.insert_cap_grant(&action_hash, 1 /* Transferable */, Some("my-tag"))
            .await
            .unwrap();

        let by_access = db
            .as_ref()
            .get_cap_grants_by_access(author.clone(), 1)
            .await
            .unwrap();
        assert_eq!(by_access.len(), 1);
        assert_eq!(by_access[0].action_hash, action_hash.get_raw_36().to_vec());

        let by_tag = db
            .as_ref()
            .get_cap_grants_by_tag(author, "my-tag")
            .await
            .unwrap();
        assert_eq!(by_tag.len(), 1);
    }

    #[tokio::test]
    async fn cap_grants_ordered_by_action_seq() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        // Insert parent Actions and grants in non-seq insertion order, so the
        // ORDER BY actually has work to do.
        for seed in [3u8, 1, 2] {
            let action = sample_action(seed);
            db.insert_action(&action, None).await.unwrap();
            db.insert_cap_grant(action.as_hash(), 1, Some("shared-tag"))
                .await
                .unwrap();
        }

        let author = AgentPubKey::from_raw_36(vec![1u8; 36]);
        let by_access = db
            .as_ref()
            .get_cap_grants_by_access(author.clone(), 1)
            .await
            .unwrap();
        assert_eq!(by_access.len(), 3);
        assert_eq!(by_access[0].action_hash, vec![1u8; 36]);
        assert_eq!(by_access[1].action_hash, vec![2u8; 36]);
        assert_eq!(by_access[2].action_hash, vec![3u8; 36]);

        let by_tag = db
            .as_ref()
            .get_cap_grants_by_tag(author, "shared-tag")
            .await
            .unwrap();
        assert_eq!(by_tag.len(), 3);
        assert_eq!(by_tag[0].action_hash, vec![1u8; 36]);
        assert_eq!(by_tag[1].action_hash, vec![2u8; 36]);
        assert_eq!(by_tag[2].action_hash, vec![3u8; 36]);
    }

    #[tokio::test]
    async fn cap_claim_roundtrip() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let author = AgentPubKey::from_raw_36(vec![5u8; 36]);
        let grantor = AgentPubKey::from_raw_36(vec![6u8; 36]);

        db.insert_cap_claim(&author, "claim-tag", &grantor, &[9u8; 32])
            .await
            .unwrap();

        let by_grantor = db
            .as_ref()
            .get_cap_claims_by_grantor(author.clone(), grantor)
            .await
            .unwrap();
        assert_eq!(by_grantor.len(), 1);
        assert_eq!(by_grantor[0].tag, "claim-tag");

        let by_tag = db
            .as_ref()
            .get_cap_claims_by_tag(author, "claim-tag")
            .await
            .unwrap();
        assert_eq!(by_tag.len(), 1);
    }

    #[tokio::test]
    async fn cap_grant_requires_action_fk() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let missing = ActionHash::from_raw_36(vec![42u8; 36]);
        let err = db
            .insert_cap_grant(&missing, 0, None)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.to_lowercase().contains("foreign key"), "got: {err}");
    }

    #[tokio::test]
    async fn chain_lock_acquire_and_read() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let author = AgentPubKey::from_raw_36(vec![7u8; 36]);
        let subject = vec![1u8; 32];

        let acquired = db
            .acquire_chain_lock(
                &author,
                &subject,
                Timestamp::from_micros(10_000),
                Timestamp::from_micros(0),
            )
            .await
            .unwrap();
        assert!(acquired);

        let lock = db
            .as_ref()
            .get_chain_lock(author.clone(), Timestamp::from_micros(5_000))
            .await
            .unwrap()
            .expect("expected active lock");
        assert_eq!(lock.subject, subject);
    }

    #[tokio::test]
    async fn chain_lock_same_subject_can_extend() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let author = AgentPubKey::from_raw_36(vec![7u8; 36]);
        let subject = vec![1u8; 32];

        assert!(db
            .acquire_chain_lock(
                &author,
                &subject,
                Timestamp::from_micros(10_000),
                Timestamp::from_micros(0),
            )
            .await
            .unwrap());
        // Same holder extends the expiry while the lock is still active.
        assert!(db
            .acquire_chain_lock(
                &author,
                &subject,
                Timestamp::from_micros(20_000),
                Timestamp::from_micros(5_000),
            )
            .await
            .unwrap());

        let lock = db
            .as_ref()
            .get_chain_lock(author, Timestamp::from_micros(5_000))
            .await
            .unwrap()
            .expect("expected lock");
        assert_eq!(lock.subject, subject);
        assert_eq!(lock.expires_at_timestamp, 20_000);
    }

    #[tokio::test]
    async fn chain_lock_different_subject_cannot_steal_active_lock() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let author = AgentPubKey::from_raw_36(vec![7u8; 36]);

        assert!(db
            .acquire_chain_lock(
                &author,
                &[1u8; 32],
                Timestamp::from_micros(10_000),
                Timestamp::from_micros(0),
            )
            .await
            .unwrap());

        // A different subject cannot steal the unexpired lock.
        let stole = db
            .acquire_chain_lock(
                &author,
                &[2u8; 32],
                Timestamp::from_micros(20_000),
                Timestamp::from_micros(5_000),
            )
            .await
            .unwrap();
        assert!(!stole);

        // Existing lock is untouched.
        let lock = db
            .as_ref()
            .get_chain_lock(author, Timestamp::from_micros(5_000))
            .await
            .unwrap()
            .expect("expected lock");
        assert_eq!(lock.subject, vec![1u8; 32]);
        assert_eq!(lock.expires_at_timestamp, 10_000);
    }

    #[tokio::test]
    async fn chain_lock_new_subject_can_acquire_after_expiry() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let author = AgentPubKey::from_raw_36(vec![7u8; 36]);

        assert!(db
            .acquire_chain_lock(
                &author,
                &[1u8; 32],
                Timestamp::from_micros(10_000),
                Timestamp::from_micros(0),
            )
            .await
            .unwrap());

        // At `now = 10_000`, the previous lock is expired (expires_at <= now), so
        // a different subject may take over.
        assert!(db
            .acquire_chain_lock(
                &author,
                &[2u8; 32],
                Timestamp::from_micros(30_000),
                Timestamp::from_micros(10_000),
            )
            .await
            .unwrap());

        let lock = db
            .as_ref()
            .get_chain_lock(author, Timestamp::from_micros(20_000))
            .await
            .unwrap()
            .expect("expected lock");
        assert_eq!(lock.subject, vec![2u8; 32]);
        assert_eq!(lock.expires_at_timestamp, 30_000);
    }

    #[tokio::test]
    async fn chain_lock_release_and_prune() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let a = AgentPubKey::from_raw_36(vec![1u8; 36]);
        let b = AgentPubKey::from_raw_36(vec![2u8; 36]);

        assert!(db
            .acquire_chain_lock(
                &a,
                &[1u8; 32],
                Timestamp::from_micros(100),
                Timestamp::from_micros(0),
            )
            .await
            .unwrap());
        assert!(db
            .acquire_chain_lock(
                &b,
                &[2u8; 32],
                Timestamp::from_micros(1_000),
                Timestamp::from_micros(0),
            )
            .await
            .unwrap());

        db.release_chain_lock(&a).await.unwrap();
        assert!(db
            .as_ref()
            .get_chain_lock(a.clone(), Timestamp::from_micros(50))
            .await
            .unwrap()
            .is_none());

        // Prune anything expired at t=500; b's lock (expires 1000) should survive.
        db.prune_expired_chain_locks(Timestamp::from_micros(500))
            .await
            .unwrap();
        assert!(db
            .as_ref()
            .get_chain_lock(b, Timestamp::from_micros(200))
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn chain_lock_expired_is_not_returned() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let author = AgentPubKey::from_raw_36(vec![9u8; 36]);
        db.acquire_chain_lock(
            &author,
            &[3u8; 32],
            Timestamp::from_micros(100),
            Timestamp::from_micros(0),
        )
        .await
        .unwrap();
        // Now is past expiry.
        assert!(db
            .as_ref()
            .get_chain_lock(author, Timestamp::from_micros(200))
            .await
            .unwrap()
            .is_none());
    }

    async fn seed_action_for_op(db: &crate::handles::DbWrite<Dht>, seed: u8) -> ActionHash {
        let action = sample_action(seed);
        db.insert_action(&action, None).await.unwrap();
        action.as_hash().clone()
    }

    fn sample_basis(seed: u8) -> AnyDhtHash {
        AnyDhtHash::from_raw_36_and_type(vec![seed; 36], holo_hash::hash_type::AnyDht::Entry)
    }

    #[tokio::test]
    async fn limbo_chain_op_roundtrip_and_state_filters() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let action_hash = seed_action_for_op(&db, 0).await;
        let op_hash = DhtOpHash::from_raw_36(vec![0xAA; 36]);

        db.insert_limbo_chain_op(InsertLimboChainOp {
            op_hash: &op_hash,
            action_hash: &action_hash,
            op_type: 1,
            basis_hash: &sample_basis(1),
            storage_center_loc: 42,
            require_receipt: true,
            when_received: Timestamp::from_micros(100),
            serialized_size: 256,
        })
        .await
        .unwrap();

        let row = db
            .as_ref()
            .get_limbo_chain_op(op_hash.clone())
            .await
            .unwrap()
            .expect("row missing");
        assert_eq!(row.op_type, 1);
        assert_eq!(row.require_receipt, 1);
        assert_eq!(row.sys_validation_status, None);

        // Appears in pending_sys.
        let pending = db
            .as_ref()
            .limbo_chain_ops_pending_sys_validation(10)
            .await
            .unwrap();
        assert_eq!(pending.len(), 1);

        // Does not appear in pending_app (sys is still NULL).
        let app_pending = db
            .as_ref()
            .limbo_chain_ops_pending_app_validation(10)
            .await
            .unwrap();
        assert!(app_pending.is_empty());

        // Flip sys to accepted via raw query so the test doesn't need an
        // update helper (workflows will add one later).
        sqlx::query("UPDATE LimboChainOp SET sys_validation_status = 1 WHERE hash = ?")
            .bind(op_hash.get_raw_36())
            .execute(db.pool())
            .await
            .unwrap();

        let app_pending = db
            .as_ref()
            .limbo_chain_ops_pending_app_validation(10)
            .await
            .unwrap();
        assert_eq!(app_pending.len(), 1);

        // Ready for integration when sys=reject, or sys=accept + app terminal.
        sqlx::query("UPDATE LimboChainOp SET app_validation_status = 1 WHERE hash = ?")
            .bind(op_hash.get_raw_36())
            .execute(db.pool())
            .await
            .unwrap();
        let ready = db
            .as_ref()
            .limbo_chain_ops_ready_for_integration(10)
            .await
            .unwrap();
        assert_eq!(ready.len(), 1);

        db.delete_limbo_chain_op(op_hash.clone()).await.unwrap();
        assert!(db
            .as_ref()
            .get_limbo_chain_op(op_hash)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn limbo_warrant_roundtrip() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let hash = DhtOpHash::from_raw_36(vec![0xBB; 36]);
        let author = AgentPubKey::from_raw_36(vec![1u8; 36]);
        let warrantee = AgentPubKey::from_raw_36(vec![2u8; 36]);
        let proof = vec![0u8; 64];

        db.insert_limbo_warrant(InsertLimboWarrant {
            hash: &hash,
            author: &author,
            timestamp: Timestamp::from_micros(10),
            warrantee: &warrantee,
            proof: &proof,
            storage_center_loc: 77,
            when_received: Timestamp::from_micros(100),
            serialized_size: 128,
        })
        .await
        .unwrap();

        let row = db
            .as_ref()
            .get_limbo_warrant(hash.clone())
            .await
            .unwrap()
            .expect("missing");
        assert_eq!(row.warrantee, warrantee.get_raw_36().to_vec());
        assert!(
            db.as_ref()
                .limbo_warrants_pending_sys_validation(10)
                .await
                .unwrap()
                .len()
                == 1
        );
        assert!(db
            .as_ref()
            .limbo_warrants_ready_for_integration(10)
            .await
            .unwrap()
            .is_empty());

        sqlx::query("UPDATE LimboWarrant SET sys_validation_status = 1 WHERE hash = ?")
            .bind(hash.get_raw_36())
            .execute(db.pool())
            .await
            .unwrap();
        assert_eq!(
            db.as_ref()
                .limbo_warrants_ready_for_integration(10)
                .await
                .unwrap()
                .len(),
            1
        );

        db.delete_limbo_warrant(hash.clone()).await.unwrap();
        assert!(db.as_ref().get_limbo_warrant(hash).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn warrant_roundtrip() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let hash = DhtOpHash::from_raw_36(vec![0xAB; 36]);
        let author = AgentPubKey::from_raw_36(vec![3u8; 36]);
        let warrantee = AgentPubKey::from_raw_36(vec![4u8; 36]);

        db.insert_warrant(InsertWarrant {
            hash: &hash,
            author: &author,
            timestamp: Timestamp::from_micros(1),
            warrantee: &warrantee,
            proof: &[9u8; 32],
            storage_center_loc: 88,
        })
        .await
        .unwrap();

        let row = db
            .as_ref()
            .get_warrant(hash.clone())
            .await
            .unwrap()
            .expect("missing");
        assert_eq!(row.warrantee, warrantee.get_raw_36().to_vec());

        let by_warrantee = db
            .as_ref()
            .get_warrants_by_warrantee(warrantee)
            .await
            .unwrap();
        assert_eq!(by_warrantee.len(), 1);
    }

    #[tokio::test]
    async fn chain_op_roundtrip() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let action_hash = seed_action_for_op(&db, 2).await;
        let op_hash = DhtOpHash::from_raw_36(vec![0xCC; 36]);
        let basis = sample_basis(5);

        db.insert_chain_op(InsertChainOp {
            op_hash: &op_hash,
            action_hash: &action_hash,
            op_type: 1,
            basis_hash: &basis,
            storage_center_loc: 99,
            validation_status: RecordValidity::Accepted,
            locally_validated: true,
            when_received: Timestamp::from_micros(10),
            when_integrated: Timestamp::from_micros(20),
            serialized_size: 512,
        })
        .await
        .unwrap();

        let row = db
            .as_ref()
            .get_chain_op(op_hash.clone())
            .await
            .unwrap()
            .expect("missing");
        assert_eq!(row.validation_status, 1);
        assert_eq!(row.locally_validated, 1);

        let by_basis = db.as_ref().get_chain_ops_by_basis(basis).await.unwrap();
        assert_eq!(by_basis.len(), 1);

        let for_action = db
            .as_ref()
            .get_chain_ops_for_action(action_hash)
            .await
            .unwrap();
        assert_eq!(for_action.len(), 1);
    }

    #[tokio::test]
    async fn chain_op_requires_action_fk() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let op_hash = DhtOpHash::from_raw_36(vec![0xDD; 36]);
        let missing = ActionHash::from_raw_36(vec![0xEE; 36]);
        let err = db
            .insert_chain_op(InsertChainOp {
                op_hash: &op_hash,
                action_hash: &missing,
                op_type: 1,
                basis_hash: &sample_basis(0),
                storage_center_loc: 0,
                validation_status: RecordValidity::Accepted,
                locally_validated: true,
                when_received: Timestamp::from_micros(10),
                when_integrated: Timestamp::from_micros(20),
                serialized_size: 0,
            })
            .await
            .unwrap_err()
            .to_string();
        assert!(err.to_lowercase().contains("foreign key"), "got: {err}");
    }

    async fn seed_chain_op(db: &crate::handles::DbWrite<Dht>, seed: u8) -> (DhtOpHash, ActionHash) {
        let action_hash = seed_action_for_op(db, seed).await;
        let op_hash = DhtOpHash::from_raw_36(vec![0xF0 + seed; 36]);
        db.insert_chain_op(InsertChainOp {
            op_hash: &op_hash,
            action_hash: &action_hash,
            op_type: 1,
            basis_hash: &sample_basis(seed),
            storage_center_loc: 0,
            validation_status: RecordValidity::Accepted,
            locally_validated: true,
            when_received: Timestamp::from_micros(1),
            when_integrated: Timestamp::from_micros(2),
            serialized_size: 0,
        })
        .await
        .unwrap();
        (op_hash, action_hash)
    }

    #[tokio::test]
    async fn chain_op_publish_roundtrip() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let (op_hash, _) = seed_chain_op(&db, 0).await;
        db.insert_chain_op_publish(&op_hash, None, None, None)
            .await
            .unwrap();

        let row = db
            .as_ref()
            .get_chain_op_publish(op_hash)
            .await
            .unwrap()
            .expect("missing");
        assert!(row.last_publish_time.is_none());
        assert!(row.receipts_complete.is_none());
        assert!(row.withhold_publish.is_none());
    }

    #[tokio::test]
    async fn warrant_publish_roundtrip() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let hash = DhtOpHash::from_raw_36(vec![0x11; 36]);
        let author = AgentPubKey::from_raw_36(vec![1u8; 36]);
        let warrantee = AgentPubKey::from_raw_36(vec![2u8; 36]);
        db.insert_warrant(InsertWarrant {
            hash: &hash,
            author: &author,
            timestamp: Timestamp::from_micros(1),
            warrantee: &warrantee,
            proof: &[0u8; 32],
            storage_center_loc: 0,
        })
        .await
        .unwrap();
        db.insert_warrant_publish(&hash, Some(Timestamp::from_micros(10)))
            .await
            .unwrap();

        let row = db
            .as_ref()
            .get_warrant_publish(hash)
            .await
            .unwrap()
            .expect("missing");
        assert_eq!(row.last_publish_time, Some(10));
    }

    #[tokio::test]
    async fn validation_receipt_roundtrip() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let (op_hash, _) = seed_chain_op(&db, 1).await;
        let receipt_hash = DhtOpHash::from_raw_36(vec![0x22; 36]);
        db.insert_validation_receipt(
            &receipt_hash,
            &op_hash,
            &[1u8; 16],
            &[2u8; 64],
            Timestamp::from_micros(42),
        )
        .await
        .unwrap();

        let rows = db.as_ref().get_validation_receipts(op_hash).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].hash, receipt_hash.get_raw_36().to_vec());
    }

    fn sample_base(seed: u8) -> AnyLinkableHash {
        AnyLinkableHash::from_raw_36_and_type(
            vec![seed; 36],
            holo_hash::hash_type::AnyLinkable::Entry,
        )
    }

    #[tokio::test]
    async fn link_roundtrip_and_cascade() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let action_hash = seed_action_for_op(&db, 0).await;
        let base = sample_base(5);

        let mut tx = db.begin().await.unwrap();
        let _ = tx
            .insert_link_index(InsertLink {
                action_hash: &action_hash,
                base_hash: &base,
                zome_index: 3,
                link_type: 7,
                tag: Some(b"tag-bytes"),
            })
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let rows = db.as_ref().get_links_by_base(base).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].zome_index, 3);
        assert_eq!(rows[0].link_type, 7);
        assert_eq!(rows[0].tag, Some(b"tag-bytes".to_vec()));

        // CASCADE: deleting the parent Action removes the Link row.
        sqlx::query("DELETE FROM Action WHERE hash = ?")
            .bind(action_hash.get_raw_36())
            .execute(db.pool())
            .await
            .unwrap();
        let rows = db.as_ref().get_links_by_base(sample_base(5)).await.unwrap();
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn deleted_link_roundtrip() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let delete_action = seed_action_for_op(&db, 1).await;
        let create_link = ActionHash::from_raw_36(vec![0x55; 36]);

        let mut tx = db.begin().await.unwrap();
        let _ = tx
            .insert_deleted_link_index(InsertDeletedLink {
                action_hash: &delete_action,
                create_link_hash: &create_link,
            })
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let rows = db.as_ref().get_deleted_links(create_link).await.unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[tokio::test]
    async fn updated_record_roundtrip() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let update_action = seed_action_for_op(&db, 2).await;
        let original = ActionHash::from_raw_36(vec![0x66; 36]);
        let original_entry = EntryHash::from_raw_36(vec![0x77; 36]);

        let mut tx = db.begin().await.unwrap();
        let _ = tx
            .insert_updated_record_index(InsertUpdatedRecord {
                action_hash: &update_action,
                original_action_hash: &original,
                original_entry_hash: &original_entry,
            })
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let rows = db
            .as_ref()
            .get_updated_records(original.clone())
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].original_entry_hash,
            original_entry.get_raw_36().to_vec()
        );
    }

    #[tokio::test]
    async fn deleted_record_roundtrip() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let delete_action = seed_action_for_op(&db, 3).await;
        let deletes_action = ActionHash::from_raw_36(vec![0x88; 36]);
        let deletes_entry = EntryHash::from_raw_36(vec![0x99; 36]);

        let mut tx = db.begin().await.unwrap();
        let _ = tx
            .insert_deleted_record_index(InsertDeletedRecord {
                action_hash: &delete_action,
                deletes_action_hash: &deletes_action,
                deletes_entry_hash: &deletes_entry,
            })
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let rows = db
            .as_ref()
            .get_deleted_records(deletes_action)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
    }

    fn sample_action_with_data(seed: u8, data: ActionData) -> SignedActionHashed {
        let action = Action {
            header: ActionHeader {
                author: AgentPubKey::from_raw_36(vec![0xAB; 36]),
                timestamp: Timestamp::from_micros(seed as i64 + 1),
                action_seq: seed as u32,
                prev_action: Some(ActionHash::from_raw_36(vec![seed.wrapping_sub(1); 36])),
            },
            data,
        };
        let hashed = HoloHashed::with_pre_hashed(action, ActionHash::from_raw_36(vec![seed; 36]));
        SignedHashed::with_presigned(hashed, Signature([seed; 64]))
    }

    #[tokio::test]
    async fn every_action_variant_roundtrips() {
        use holochain_integrity_types::dht_v2::*;

        let entry_hash = EntryHash::from_raw_36(vec![9u8; 36]);
        let action_hash = ActionHash::from_raw_36(vec![10u8; 36]);
        let any_link = AnyLinkableHash::from_raw_36_and_type(
            vec![11u8; 36],
            holo_hash::hash_type::AnyLinkable::Entry,
        );
        let entry_type = holochain_integrity_types::EntryType::AgentPubKey;

        let cases: Vec<(u8, ActionData)> = vec![
            (
                1,
                ActionData::Dna(DnaData {
                    dna_hash: DnaHash::from_raw_36(vec![0u8; 36]),
                }),
            ),
            (
                2,
                ActionData::AgentValidationPkg(AgentValidationPkgData {
                    membrane_proof: None,
                }),
            ),
            (3, ActionData::InitZomesComplete(InitZomesCompleteData {})),
            (
                4,
                ActionData::Create(CreateData {
                    entry_type: entry_type.clone(),
                    entry_hash: entry_hash.clone(),
                }),
            ),
            (
                5,
                ActionData::Update(UpdateData {
                    original_action_address: action_hash.clone(),
                    original_entry_address: entry_hash.clone(),
                    entry_type: entry_type.clone(),
                    entry_hash: entry_hash.clone(),
                }),
            ),
            (
                6,
                ActionData::Delete(DeleteData {
                    deletes_address: action_hash.clone(),
                    deletes_entry_address: entry_hash.clone(),
                }),
            ),
            (
                7,
                ActionData::CreateLink(CreateLinkData {
                    base_address: any_link.clone(),
                    target_address: any_link.clone(),
                    zome_index: holochain_integrity_types::action::ZomeIndex(0),
                    link_type: holochain_integrity_types::link::LinkType(0),
                    tag: holochain_integrity_types::link::LinkTag(vec![]),
                }),
            ),
            (
                8,
                ActionData::DeleteLink(DeleteLinkData {
                    base_address: any_link,
                    link_add_address: action_hash,
                }),
            ),
        ];

        let db = test_open_db(dht_db_id()).await.unwrap();
        for (seed, data) in cases {
            let action = sample_action_with_data(seed, data);
            db.insert_action(&action, None).await.unwrap();
            let fetched = db
                .as_ref()
                .get_action(action.as_hash().clone())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(fetched, action);
            assert_eq!(
                i64::from(fetched.hashed.content.data.action_type()),
                seed as i64
            );
            assert_eq!(fetched.signature().0, [seed; 64]);
        }
    }

    #[tokio::test]
    async fn set_limbo_chain_op_sys_validation_status_updates() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let action_hash = seed_action_for_op(&db, 0).await;
        let op_hash = DhtOpHash::from_raw_36(vec![0xAA; 36]);
        db.insert_limbo_chain_op(InsertLimboChainOp {
            op_hash: &op_hash,
            action_hash: &action_hash,
            op_type: 1,
            basis_hash: &sample_basis(1),
            storage_center_loc: 42,
            require_receipt: true,
            when_received: Timestamp::from_micros(100),
            serialized_size: 256,
        })
        .await
        .unwrap();

        let updated = db
            .set_limbo_chain_op_sys_validation_status(&op_hash, Some(1))
            .await
            .unwrap();
        assert_eq!(updated, 1);

        let row = db
            .as_ref()
            .get_limbo_chain_op(op_hash.clone())
            .await
            .unwrap()
            .expect("row");
        assert_eq!(row.sys_validation_status, Some(1));

        // Unknown hash → 0 rows.
        let missing = DhtOpHash::from_raw_36(vec![0xFF; 36]);
        let updated = db
            .set_limbo_chain_op_sys_validation_status(&missing, Some(2))
            .await
            .unwrap();
        assert_eq!(updated, 0);
    }

    #[tokio::test]
    async fn set_limbo_chain_op_app_validation_status_updates() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let action_hash = seed_action_for_op(&db, 0).await;
        let op_hash = DhtOpHash::from_raw_36(vec![0xAB; 36]);
        db.insert_limbo_chain_op(InsertLimboChainOp {
            op_hash: &op_hash,
            action_hash: &action_hash,
            op_type: 1,
            basis_hash: &sample_basis(1),
            storage_center_loc: 0,
            require_receipt: true,
            when_received: Timestamp::from_micros(1),
            serialized_size: 100,
        })
        .await
        .unwrap();

        // sys_validation_status must be set first (set-once ordering constraint).
        db.set_limbo_chain_op_sys_validation_status(&op_hash, Some(1))
            .await
            .unwrap();

        let updated = db
            .set_limbo_chain_op_app_validation_status(&op_hash, Some(1))
            .await
            .unwrap();
        assert_eq!(updated, 1);
        let row = db
            .as_ref()
            .get_limbo_chain_op(op_hash)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.app_validation_status, Some(1));
    }

    #[tokio::test]
    async fn set_limbo_warrant_sys_validation_status_updates() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let hash = DhtOpHash::from_raw_36(vec![0xBB; 36]);
        let author = AgentPubKey::from_raw_36(vec![1u8; 36]);
        let warrantee = AgentPubKey::from_raw_36(vec![2u8; 36]);
        db.insert_limbo_warrant(InsertLimboWarrant {
            hash: &hash,
            author: &author,
            timestamp: Timestamp::from_micros(10),
            warrantee: &warrantee,
            proof: &[0u8; 64],
            storage_center_loc: 77,
            when_received: Timestamp::from_micros(100),
            serialized_size: 128,
        })
        .await
        .unwrap();

        let updated = db
            .set_limbo_warrant_sys_validation_status(&hash, Some(1))
            .await
            .unwrap();
        assert_eq!(updated, 1);

        let row = db.as_ref().get_limbo_warrant(hash).await.unwrap().unwrap();
        assert_eq!(row.sys_validation_status, Some(1));
    }

    #[tokio::test]
    async fn promote_limbo_chain_op_round_trip() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let action_hash = seed_action_for_op(&db, 0).await;
        let op_hash = DhtOpHash::from_raw_36(vec![0xA0; 36]);
        db.insert_limbo_chain_op(InsertLimboChainOp {
            op_hash: &op_hash,
            action_hash: &action_hash,
            op_type: 1,
            basis_hash: &sample_basis(1),
            storage_center_loc: 42,
            require_receipt: true,
            when_received: Timestamp::from_micros(100),
            serialized_size: 256,
        })
        .await
        .unwrap();
        db.set_limbo_chain_op_sys_validation_status(&op_hash, Some(1))
            .await
            .unwrap();
        db.set_limbo_chain_op_app_validation_status(&op_hash, Some(1))
            .await
            .unwrap();

        let promoted = db
            .promote_limbo_chain_op(
                &op_hash,
                RecordValidity::Accepted,
                Timestamp::from_micros(500),
            )
            .await
            .unwrap();
        assert!(promoted);

        assert!(db
            .as_ref()
            .get_limbo_chain_op(op_hash.clone())
            .await
            .unwrap()
            .is_none());
        let row = db
            .as_ref()
            .get_chain_op(op_hash)
            .await
            .unwrap()
            .expect("chain op");
        assert_eq!(row.when_integrated, 500);
        assert_eq!(row.serialized_size, 256);
        assert_eq!(row.validation_status, i64::from(RecordValidity::Accepted));
        assert_eq!(row.locally_validated, 1); // promoted from limbo = locally validated
        assert_eq!(row.when_received, 100);
        assert_eq!(row.storage_center_loc, 42);
    }

    #[tokio::test]
    async fn promote_limbo_chain_op_missing_returns_false() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let op_hash = DhtOpHash::from_raw_36(vec![0xA1; 36]);
        let promoted = db
            .promote_limbo_chain_op(
                &op_hash,
                RecordValidity::Accepted,
                Timestamp::from_micros(500),
            )
            .await
            .unwrap();
        assert!(!promoted);
    }

    #[tokio::test]
    async fn promote_limbo_warrant_round_trip() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let hash = DhtOpHash::from_raw_36(vec![0xA2; 36]);
        let author = AgentPubKey::from_raw_36(vec![1u8; 36]);
        let warrantee = AgentPubKey::from_raw_36(vec![2u8; 36]);
        db.insert_limbo_warrant(InsertLimboWarrant {
            hash: &hash,
            author: &author,
            timestamp: Timestamp::from_micros(10),
            warrantee: &warrantee,
            proof: &[5u8; 64],
            storage_center_loc: 77,
            when_received: Timestamp::from_micros(100),
            serialized_size: 128,
        })
        .await
        .unwrap();

        let promoted = db.promote_limbo_warrant(&hash).await.unwrap();
        assert!(promoted);

        assert!(db
            .as_ref()
            .get_limbo_warrant(hash.clone())
            .await
            .unwrap()
            .is_none());
        let row = db
            .as_ref()
            .get_warrant(hash)
            .await
            .unwrap()
            .expect("warrant");
        assert_eq!(row.warrantee, warrantee.get_raw_36().to_vec());
        assert_eq!(row.proof, vec![5u8; 64]);
        assert_eq!(row.storage_center_loc, 77);
    }

    #[tokio::test]
    async fn set_chain_op_last_publish_time_updates() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let (op_hash, _) = seed_chain_op(&db, 0).await;
        db.insert_chain_op_publish(&op_hash, None, None, None)
            .await
            .unwrap();

        let updated = db
            .set_chain_op_last_publish_time(&op_hash, Timestamp::from_micros(42))
            .await
            .unwrap();
        assert_eq!(updated, 1);

        let row = db
            .as_ref()
            .get_chain_op_publish(op_hash)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.last_publish_time, Some(42));
    }

    #[tokio::test]
    async fn clear_chain_op_withhold_publish_round_trip() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let (op_hash, _) = seed_chain_op(&db, 0).await;
        db.insert_chain_op_publish(&op_hash, None, None, Some(true))
            .await
            .unwrap();

        let row = db
            .as_ref()
            .get_chain_op_publish(op_hash.clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.withhold_publish, Some(1));

        let updated = db.clear_chain_op_withhold_publish(&op_hash).await.unwrap();
        assert_eq!(updated, 1);

        let row = db
            .as_ref()
            .get_chain_op_publish(op_hash)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.withhold_publish, None);
    }

    #[tokio::test]
    async fn set_chain_op_validation_status_updates() {
        // The transition only applies to network-cached ops; seed one with
        // locally_validated = false.
        let db = test_open_db(dht_db_id()).await.unwrap();
        let action_hash = seed_action_for_op(&db, 0).await;
        let op_hash = DhtOpHash::from_raw_36(vec![0xF0; 36]);
        db.insert_chain_op(InsertChainOp {
            op_hash: &op_hash,
            action_hash: &action_hash,
            op_type: 1,
            basis_hash: &sample_basis(0),
            storage_center_loc: 0,
            validation_status: RecordValidity::Accepted,
            locally_validated: false,
            when_received: Timestamp::from_micros(1),
            when_integrated: Timestamp::from_micros(2),
            serialized_size: 0,
        })
        .await
        .unwrap();

        let updated = db
            .set_chain_op_validation_status(&op_hash, RecordValidity::Rejected)
            .await
            .unwrap();
        assert_eq!(updated, 1);
        let row = db.as_ref().get_chain_op(op_hash).await.unwrap().unwrap();
        assert_eq!(row.validation_status, i64::from(RecordValidity::Rejected));
    }

    #[tokio::test]
    async fn set_chain_op_validation_status_skips_locally_validated() {
        // Locally validated ops never change status through this path.
        let db = test_open_db(dht_db_id()).await.unwrap();
        let (op_hash, _) = seed_chain_op(&db, 0).await; // locally_validated = true
        let updated = db
            .set_chain_op_validation_status(&op_hash, RecordValidity::Rejected)
            .await
            .unwrap();
        assert_eq!(updated, 0);
        let row = db.as_ref().get_chain_op(op_hash).await.unwrap().unwrap();
        assert_eq!(row.validation_status, i64::from(RecordValidity::Accepted));
    }

    #[tokio::test]
    async fn set_chain_op_receipts_complete_round_trip() {
        let db = test_open_db(dht_db_id()).await.unwrap();

        let action = sample_action(7);
        db.insert_action(&action, Some(RecordValidity::Accepted))
            .await
            .unwrap();

        let op_hash = DhtOpHash::from_raw_36(vec![9u8; 36]);
        let basis_hash =
            AnyDhtHash::from_raw_36_and_type(vec![9u8; 36], holo_hash::hash_type::AnyDht::Action);

        db.insert_chain_op(InsertChainOp {
            op_hash: &op_hash,
            action_hash: action.as_hash(),
            op_type: 1,
            basis_hash: &basis_hash,
            storage_center_loc: 0,
            validation_status: RecordValidity::Accepted,
            locally_validated: true,
            when_received: Timestamp::from_micros(1),
            when_integrated: Timestamp::from_micros(1),
            serialized_size: 0,
        })
        .await
        .unwrap();

        db.insert_chain_op_publish(&op_hash, None, None, None)
            .await
            .unwrap();
        let _ = db.set_chain_op_receipts_complete(&op_hash).await.unwrap();

        let row = db
            .as_ref()
            .get_chain_op_publish(op_hash)
            .await
            .unwrap()
            .expect("row");
        assert_eq!(row.receipts_complete, Some(1));
    }
}
