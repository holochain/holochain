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
pub use inner::limbo_chain_op::LimboChainOpJoinedRow;
pub use inner::limbo_warrant::InsertLimboWarrant;
pub use inner::link::InsertLink;
pub use inner::remove_countersigning_session::RemoveCountersigningSessionOutcome;
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

    #[tokio::test]
    async fn entry_batch_fetch_by_hashes() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let author = AgentPubKey::from_raw_36(vec![9u8; 36]);

        // Two public entries and one private entry owned by `author`.
        let (pub_hash_a, pub_entry_a) = sample_entry(20);
        let (pub_hash_b, pub_entry_b) = sample_entry(21);
        let (priv_hash, priv_entry) = sample_entry(22);
        db.insert_entry(&pub_hash_a, &pub_entry_a).await.unwrap();
        db.insert_entry(&pub_hash_b, &pub_entry_b).await.unwrap();
        db.insert_private_entry(&priv_hash, &author, &priv_entry)
            .await
            .unwrap();

        // A hash that was never inserted.
        let missing_hash = EntryHash::from_raw_36(vec![99u8; 36]);

        // With the owning author, the private entry resolves alongside the
        // public ones; the missing hash is simply absent from the map.
        let map = db
            .as_ref()
            .get_entries_by_hashes(
                &[
                    pub_hash_a.clone(),
                    pub_hash_b.clone(),
                    priv_hash.clone(),
                    missing_hash.clone(),
                ],
                Some(&author),
            )
            .await
            .unwrap();
        assert_eq!(map.len(), 3);
        assert_eq!(map.get(&pub_hash_a), Some(&pub_entry_a));
        assert_eq!(map.get(&pub_hash_b), Some(&pub_entry_b));
        assert_eq!(map.get(&priv_hash), Some(&priv_entry));
        assert_eq!(map.get(&missing_hash), None);

        // Without an author the private entry is excluded, public entries still
        // resolve, and duplicate input hashes collapse to one map entry.
        let map_public = db
            .as_ref()
            .get_entries_by_hashes(
                &[pub_hash_a.clone(), pub_hash_a.clone(), priv_hash.clone()],
                None,
            )
            .await
            .unwrap();
        assert_eq!(map_public.len(), 1);
        assert_eq!(map_public.get(&pub_hash_a), Some(&pub_entry_a));
        assert_eq!(map_public.get(&priv_hash), None);
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

    fn sample_basis(seed: u8) -> AnyLinkableHash {
        AnyLinkableHash::from_raw_36_and_type(
            vec![seed; 36],
            holo_hash::hash_type::AnyLinkable::Entry,
        )
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
            signature: &[7u8; 64],
            reason: Some("invalid chain op"),
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
        assert_eq!(row.signature, vec![7u8; 64]);
        assert_eq!(row.reason.as_deref(), Some("invalid chain op"));
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

        sqlx::query("UPDATE LimboWarrantOp SET sys_validation_status = 1 WHERE hash = ?")
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
            signature: &[8u8; 64],
            reason: Some("rejected by app validation"),
            storage_center_loc: 88,
            when_received: Timestamp::from_micros(40),
            when_integrated: Timestamp::from_micros(50),
            validation_status: 1,
            serialized_size: 128,
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
        assert_eq!(row.when_received, 40);
        assert_eq!(row.when_integrated, 50);
        assert_eq!(row.serialized_size, 128);
        assert_eq!(row.reason.as_deref(), Some("rejected by app validation"));

        let by_warrantee = db
            .as_ref()
            .get_warrants_by_warrantee(warrantee)
            .await
            .unwrap();
        assert_eq!(by_warrantee.len(), 1);
    }

    #[tokio::test]
    async fn warrants_by_author_and_op_validation_status() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let author_a = AgentPubKey::from_raw_36(vec![3u8; 36]);
        let author_b = AgentPubKey::from_raw_36(vec![5u8; 36]);
        let warrantee = AgentPubKey::from_raw_36(vec![4u8; 36]);
        let hash_a = DhtOpHash::from_raw_36(vec![0xA1; 36]);
        let hash_b = DhtOpHash::from_raw_36(vec![0xB2; 36]);

        for (hash, author, status) in [(&hash_a, &author_a, 2), (&hash_b, &author_b, 1)] {
            db.insert_warrant(InsertWarrant {
                hash,
                author,
                timestamp: Timestamp::from_micros(1),
                warrantee: &warrantee,
                proof: &[9u8; 32],
                signature: &[8u8; 64],
                reason: None,
                storage_center_loc: 0,
                when_received: Timestamp::from_micros(1),
                when_integrated: Timestamp::from_micros(2),
                validation_status: status,
                serialized_size: 0,
            })
            .await
            .unwrap();
        }

        let by_a = db
            .as_ref()
            .get_warrants_by_author(author_a.clone())
            .await
            .unwrap();
        assert_eq!(by_a.len(), 1);
        assert_eq!(by_a[0].author, author_a.get_raw_36().to_vec());

        assert_eq!(
            db.as_ref()
                .warrant_op_validation_status(&hash_a)
                .await
                .unwrap(),
            Some(2)
        );
        assert_eq!(
            db.as_ref()
                .warrant_op_validation_status(&hash_b)
                .await
                .unwrap(),
            Some(1)
        );
        assert_eq!(
            db.as_ref()
                .warrant_op_validation_status(&DhtOpHash::from_raw_36(vec![0x99; 36]))
                .await
                .unwrap(),
            None
        );

        // A limbo (not-yet-integrated) warrant whose sys-validation has been
        // decided is read from LimboWarrantOp.
        let limbo_hash = DhtOpHash::from_raw_36(vec![0xC3; 36]);
        db.insert_limbo_warrant(InsertLimboWarrant {
            hash: &limbo_hash,
            author: &author_a,
            timestamp: Timestamp::from_micros(1),
            warrantee: &warrantee,
            proof: &[9u8; 32],
            signature: &[8u8; 64],
            reason: None,
            storage_center_loc: 0,
            when_received: Timestamp::from_micros(1),
            serialized_size: 0,
        })
        .await
        .unwrap();
        assert_eq!(
            db.as_ref()
                .warrant_op_validation_status(&limbo_hash)
                .await
                .unwrap(),
            None,
            "pending limbo warrant has no decided status"
        );
        db.set_limbo_warrant_sys_validation_status(&limbo_hash, Some(2))
            .await
            .unwrap();
        assert_eq!(
            db.as_ref()
                .warrant_op_validation_status(&limbo_hash)
                .await
                .unwrap(),
            Some(2)
        );

        // author_a now has both an integrated and a limbo warrant; both are
        // listed.
        let by_a = db
            .as_ref()
            .get_warrants_by_author(author_a.clone())
            .await
            .unwrap();
        assert_eq!(by_a.len(), 2);
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
            require_receipt: false,
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

        let by_basis = db
            .as_ref()
            .get_chain_ops_by_basis(AnyDhtHash::try_from(basis).unwrap())
            .await
            .unwrap();
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
                require_receipt: false,
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
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            when_integrated: Timestamp::from_micros(2),
            serialized_size: 0,
        })
        .await
        .unwrap();
        (op_hash, action_hash)
    }

    #[tokio::test]
    async fn limbo_state_counts_split_and_exclude_cached() {
        let db = test_open_db(dht_db_id()).await.unwrap();

        // One cached ChainOp (locally_validated = 0) — must NOT count as
        // integrated.
        let cached_action = seed_action_for_op(&db, 1).await;
        db.insert_chain_op(InsertChainOp {
            op_hash: &DhtOpHash::from_raw_36(vec![0x10; 36]),
            action_hash: &cached_action,
            op_type: 1,
            basis_hash: &sample_basis(1),
            storage_center_loc: 0,
            validation_status: RecordValidity::Accepted,
            locally_validated: false,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            when_integrated: Timestamp::from_micros(2),
            serialized_size: 0,
        })
        .await
        .unwrap();

        // One integrated ChainOp (locally_validated = 1).
        let integrated_action = seed_action_for_op(&db, 2).await;
        db.insert_chain_op(InsertChainOp {
            op_hash: &DhtOpHash::from_raw_36(vec![0x20; 36]),
            action_hash: &integrated_action,
            op_type: 1,
            basis_hash: &sample_basis(2),
            storage_center_loc: 0,
            validation_status: RecordValidity::Accepted,
            locally_validated: true,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            when_integrated: Timestamp::from_micros(2),
            serialized_size: 0,
        })
        .await
        .unwrap();

        // One pending LimboChainOp (sys_validation_status NULL) →
        // validation_limbo.
        let pending_action = seed_action_for_op(&db, 3).await;
        db.insert_limbo_chain_op(InsertLimboChainOp {
            op_hash: &DhtOpHash::from_raw_36(vec![0x30; 36]),
            action_hash: &pending_action,
            op_type: 1,
            basis_hash: &sample_basis(3),
            storage_center_loc: 0,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            serialized_size: 0,
        })
        .await
        .unwrap();

        // One ready LimboChainOp (sys = 1, app = 1) → integration_limbo.
        let ready_action = seed_action_for_op(&db, 4).await;
        let ready_op = DhtOpHash::from_raw_36(vec![0x40; 36]);
        db.insert_limbo_chain_op(InsertLimboChainOp {
            op_hash: &ready_op,
            action_hash: &ready_action,
            op_type: 1,
            basis_hash: &sample_basis(4),
            storage_center_loc: 0,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            serialized_size: 0,
        })
        .await
        .unwrap();
        db.set_limbo_chain_op_sys_validation_status(&ready_op, Some(1))
            .await
            .unwrap();
        db.set_limbo_chain_op_app_validation_status(&ready_op, Some(1))
            .await
            .unwrap();

        let (validation_limbo, integration_limbo, integrated) =
            db.as_ref().limbo_state_counts().await.unwrap();
        assert_eq!(validation_limbo, 1);
        assert_eq!(integration_limbo, 1);
        // Only the locally-validated ChainOp counts; the cached op is excluded.
        assert_eq!(integrated, 1);
    }

    #[tokio::test]
    async fn count_valid_integrated_ops_counts_only_locally_validated_accepted_chain_ops() {
        let db = test_open_db(dht_db_id()).await.unwrap();

        // Cached (locally_validated = 0) accepted ChainOp — excluded.
        let cached_action = seed_action_for_op(&db, 1).await;
        db.insert_chain_op(InsertChainOp {
            op_hash: &DhtOpHash::from_raw_36(vec![0x11; 36]),
            action_hash: &cached_action,
            op_type: 1,
            basis_hash: &sample_basis(1),
            storage_center_loc: 0,
            validation_status: RecordValidity::Accepted,
            locally_validated: false,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            when_integrated: Timestamp::from_micros(2),
            serialized_size: 0,
        })
        .await
        .unwrap();

        // Integrated accepted ChainOp — counted.
        let valid_action = seed_action_for_op(&db, 2).await;
        db.insert_chain_op(InsertChainOp {
            op_hash: &DhtOpHash::from_raw_36(vec![0x22; 36]),
            action_hash: &valid_action,
            op_type: 1,
            basis_hash: &sample_basis(2),
            storage_center_loc: 0,
            validation_status: RecordValidity::Accepted,
            locally_validated: true,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            when_integrated: Timestamp::from_micros(2),
            serialized_size: 0,
        })
        .await
        .unwrap();

        // Integrated rejected ChainOp — excluded.
        let rejected_action = seed_action_for_op(&db, 3).await;
        db.insert_chain_op(InsertChainOp {
            op_hash: &DhtOpHash::from_raw_36(vec![0x33; 36]),
            action_hash: &rejected_action,
            op_type: 1,
            basis_hash: &sample_basis(3),
            storage_center_loc: 0,
            validation_status: RecordValidity::Rejected,
            locally_validated: true,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            when_integrated: Timestamp::from_micros(2),
            serialized_size: 0,
        })
        .await
        .unwrap();

        // A still-in-limbo op — excluded (not integrated).
        let limbo_action = seed_action_for_op(&db, 4).await;
        db.insert_limbo_chain_op(InsertLimboChainOp {
            op_hash: &DhtOpHash::from_raw_36(vec![0x44; 36]),
            action_hash: &limbo_action,
            op_type: 1,
            basis_hash: &sample_basis(4),
            storage_center_loc: 0,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            serialized_size: 0,
        })
        .await
        .unwrap();

        let count = db.as_ref().count_valid_integrated_ops().await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn count_valid_not_integrated_ops_counts_fully_validated_limbo_chain_ops() {
        let db = test_open_db(dht_db_id()).await.unwrap();

        // Fully validated (sys = 1, app = 1) limbo op — counted.
        let valid_action = seed_action_for_op(&db, 1).await;
        let valid_op = DhtOpHash::from_raw_36(vec![0x11; 36]);
        db.insert_limbo_chain_op(InsertLimboChainOp {
            op_hash: &valid_op,
            action_hash: &valid_action,
            op_type: 1,
            basis_hash: &sample_basis(1),
            storage_center_loc: 0,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            serialized_size: 0,
        })
        .await
        .unwrap();
        db.set_limbo_chain_op_sys_validation_status(&valid_op, Some(1))
            .await
            .unwrap();
        db.set_limbo_chain_op_app_validation_status(&valid_op, Some(1))
            .await
            .unwrap();

        // Sys accepted but app still pending — excluded.
        let sys_only_action = seed_action_for_op(&db, 2).await;
        let sys_only_op = DhtOpHash::from_raw_36(vec![0x22; 36]);
        db.insert_limbo_chain_op(InsertLimboChainOp {
            op_hash: &sys_only_op,
            action_hash: &sys_only_action,
            op_type: 1,
            basis_hash: &sample_basis(2),
            storage_center_loc: 0,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            serialized_size: 0,
        })
        .await
        .unwrap();
        db.set_limbo_chain_op_sys_validation_status(&sys_only_op, Some(1))
            .await
            .unwrap();

        // Sys rejected — excluded.
        let rejected_action = seed_action_for_op(&db, 3).await;
        let rejected_op = DhtOpHash::from_raw_36(vec![0x33; 36]);
        db.insert_limbo_chain_op(InsertLimboChainOp {
            op_hash: &rejected_op,
            action_hash: &rejected_action,
            op_type: 1,
            basis_hash: &sample_basis(3),
            storage_center_loc: 0,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            serialized_size: 0,
        })
        .await
        .unwrap();
        db.set_limbo_chain_op_sys_validation_status(&rejected_op, Some(2))
            .await
            .unwrap();

        // An already-integrated accepted op — excluded (it is integrated, not
        // awaiting integration).
        let integrated_action = seed_action_for_op(&db, 4).await;
        db.insert_chain_op(InsertChainOp {
            op_hash: &DhtOpHash::from_raw_36(vec![0x44; 36]),
            action_hash: &integrated_action,
            op_type: 1,
            basis_hash: &sample_basis(4),
            storage_center_loc: 0,
            validation_status: RecordValidity::Accepted,
            locally_validated: true,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            when_integrated: Timestamp::from_micros(2),
            serialized_size: 0,
        })
        .await
        .unwrap();

        let count = db.as_ref().count_valid_not_integrated_ops().await.unwrap();
        assert_eq!(count, 1);
    }

    async fn seed_action_for_op_with_author(
        db: &crate::handles::DbWrite<Dht>,
        seed: u8,
        author: &AgentPubKey,
    ) -> ActionHash {
        let action = Action {
            header: ActionHeader {
                author: author.clone(),
                timestamp: Timestamp::from_micros(1_000_000 + seed as i64),
                action_seq: seed as u32,
                prev_action: Some(ActionHash::from_raw_36(vec![seed.wrapping_sub(1); 36])),
            },
            data: ActionData::InitZomesComplete(InitZomesCompleteData {}),
        };
        let hashed = HoloHashed::with_pre_hashed(action, ActionHash::from_raw_36(vec![seed; 36]));
        let signed = SignedHashed::with_presigned(hashed, Signature([seed; 64]));
        db.insert_action(&signed, None).await.unwrap();
        signed.as_hash().clone()
    }

    #[tokio::test]
    async fn count_pending_ops_for_author_counts_only_that_authors_limbo_ops() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        // sample_action / seed_action_for_op author is all-ones.
        let author_a = AgentPubKey::from_raw_36(vec![1u8; 36]);
        let author_b = AgentPubKey::from_raw_36(vec![0xB2; 36]);

        // Author A: two pending limbo ops.
        for (seed, op_byte) in [(1u8, 0x11u8), (2, 0x22)] {
            let action = seed_action_for_op(&db, seed).await;
            db.insert_limbo_chain_op(InsertLimboChainOp {
                op_hash: &DhtOpHash::from_raw_36(vec![op_byte; 36]),
                action_hash: &action,
                op_type: 1,
                basis_hash: &sample_basis(seed),
                storage_center_loc: 0,
                require_receipt: false,
                when_received: Timestamp::from_micros(1),
                serialized_size: 0,
            })
            .await
            .unwrap();
        }

        // Author A: one already-integrated chain op (excluded — not pending).
        let a_integrated = seed_action_for_op(&db, 4).await;
        db.insert_chain_op(InsertChainOp {
            op_hash: &DhtOpHash::from_raw_36(vec![0x44; 36]),
            action_hash: &a_integrated,
            op_type: 1,
            basis_hash: &sample_basis(4),
            storage_center_loc: 0,
            validation_status: RecordValidity::Accepted,
            locally_validated: true,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            when_integrated: Timestamp::from_micros(2),
            serialized_size: 0,
        })
        .await
        .unwrap();

        // Author B: one pending limbo op (excluded from A's count).
        let b_pending = seed_action_for_op_with_author(&db, 3, &author_b).await;
        db.insert_limbo_chain_op(InsertLimboChainOp {
            op_hash: &DhtOpHash::from_raw_36(vec![0x33; 36]),
            action_hash: &b_pending,
            op_type: 1,
            basis_hash: &sample_basis(3),
            storage_center_loc: 0,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            serialized_size: 0,
        })
        .await
        .unwrap();

        assert_eq!(
            db.as_ref()
                .count_pending_ops_for_author(&author_a)
                .await
                .unwrap(),
            2
        );
        assert_eq!(
            db.as_ref()
                .count_pending_ops_for_author(&author_b)
                .await
                .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn rejected_integrated_op_hashes_returns_only_locally_validated_rejected() {
        let db = test_open_db(dht_db_id()).await.unwrap();

        // Integrated accepted — excluded.
        let accepted = seed_action_for_op(&db, 1).await;
        db.insert_chain_op(InsertChainOp {
            op_hash: &DhtOpHash::from_raw_36(vec![0x11; 36]),
            action_hash: &accepted,
            op_type: 1,
            basis_hash: &sample_basis(1),
            storage_center_loc: 0,
            validation_status: RecordValidity::Accepted,
            locally_validated: true,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            when_integrated: Timestamp::from_micros(2),
            serialized_size: 0,
        })
        .await
        .unwrap();

        // Integrated rejected — included.
        let rejected_op = DhtOpHash::from_raw_36(vec![0x22; 36]);
        let rejected = seed_action_for_op(&db, 2).await;
        db.insert_chain_op(InsertChainOp {
            op_hash: &rejected_op,
            action_hash: &rejected,
            op_type: 1,
            basis_hash: &sample_basis(2),
            storage_center_loc: 0,
            validation_status: RecordValidity::Rejected,
            locally_validated: true,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            when_integrated: Timestamp::from_micros(2),
            serialized_size: 0,
        })
        .await
        .unwrap();

        // Cached rejected (locally_validated = 0) — excluded.
        let cached = seed_action_for_op(&db, 3).await;
        db.insert_chain_op(InsertChainOp {
            op_hash: &DhtOpHash::from_raw_36(vec![0x33; 36]),
            action_hash: &cached,
            op_type: 1,
            basis_hash: &sample_basis(3),
            storage_center_loc: 0,
            validation_status: RecordValidity::Rejected,
            locally_validated: false,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            when_integrated: Timestamp::from_micros(2),
            serialized_size: 0,
        })
        .await
        .unwrap();

        let hashes = db.as_ref().rejected_integrated_op_hashes().await.unwrap();
        assert_eq!(hashes, vec![rejected_op]);
    }

    #[tokio::test]
    async fn count_all_ops_counts_integrated_and_limbo_chain_ops() {
        let db = test_open_db(dht_db_id()).await.unwrap();

        let integrated_action = seed_action_for_op(&db, 1).await;
        db.insert_chain_op(InsertChainOp {
            op_hash: &DhtOpHash::from_raw_36(vec![0x11; 36]),
            action_hash: &integrated_action,
            op_type: 1,
            basis_hash: &sample_basis(1),
            storage_center_loc: 0,
            validation_status: RecordValidity::Accepted,
            locally_validated: true,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            when_integrated: Timestamp::from_micros(2),
            serialized_size: 0,
        })
        .await
        .unwrap();

        let limbo_action = seed_action_for_op(&db, 2).await;
        db.insert_limbo_chain_op(InsertLimboChainOp {
            op_hash: &DhtOpHash::from_raw_36(vec![0x22; 36]),
            action_hash: &limbo_action,
            op_type: 1,
            basis_hash: &sample_basis(2),
            storage_center_loc: 0,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            serialized_size: 0,
        })
        .await
        .unwrap();

        assert_eq!(db.as_ref().count_all_ops().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn op_requires_receipt_reflects_chain_op_flag() {
        let db = test_open_db(dht_db_id()).await.unwrap();

        let needs_action = seed_action_for_op(&db, 1).await;
        let needs = DhtOpHash::from_raw_36(vec![0x11; 36]);
        db.insert_chain_op(InsertChainOp {
            op_hash: &needs,
            action_hash: &needs_action,
            op_type: 1,
            basis_hash: &sample_basis(1),
            storage_center_loc: 0,
            validation_status: RecordValidity::Accepted,
            locally_validated: true,
            require_receipt: true,
            when_received: Timestamp::from_micros(1),
            when_integrated: Timestamp::from_micros(2),
            serialized_size: 0,
        })
        .await
        .unwrap();

        let no_need_action = seed_action_for_op(&db, 2).await;
        let no_need = DhtOpHash::from_raw_36(vec![0x22; 36]);
        db.insert_chain_op(InsertChainOp {
            op_hash: &no_need,
            action_hash: &no_need_action,
            op_type: 1,
            basis_hash: &sample_basis(2),
            storage_center_loc: 0,
            validation_status: RecordValidity::Accepted,
            locally_validated: true,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            when_integrated: Timestamp::from_micros(2),
            serialized_size: 0,
        })
        .await
        .unwrap();

        assert!(db.as_ref().op_requires_receipt(&needs).await.unwrap());
        assert!(!db.as_ref().op_requires_receipt(&no_need).await.unwrap());
    }

    #[tokio::test]
    async fn limbo_op_exists_only_for_unintegrated_ops() {
        let db = test_open_db(dht_db_id()).await.unwrap();

        let limbo_action = seed_action_for_op(&db, 1).await;
        let limbo_op = DhtOpHash::from_raw_36(vec![0x11; 36]);
        db.insert_limbo_chain_op(InsertLimboChainOp {
            op_hash: &limbo_op,
            action_hash: &limbo_action,
            op_type: 1,
            basis_hash: &sample_basis(1),
            storage_center_loc: 0,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            serialized_size: 0,
        })
        .await
        .unwrap();

        let integrated_action = seed_action_for_op(&db, 2).await;
        let integrated_op = DhtOpHash::from_raw_36(vec![0x22; 36]);
        db.insert_chain_op(InsertChainOp {
            op_hash: &integrated_op,
            action_hash: &integrated_action,
            op_type: 1,
            basis_hash: &sample_basis(2),
            storage_center_loc: 0,
            validation_status: RecordValidity::Accepted,
            locally_validated: true,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            when_integrated: Timestamp::from_micros(2),
            serialized_size: 0,
        })
        .await
        .unwrap();

        assert!(db.as_ref().limbo_op_exists(&limbo_op).await.unwrap());
        assert!(!db.as_ref().limbo_op_exists(&integrated_op).await.unwrap());
        assert!(!db
            .as_ref()
            .limbo_op_exists(&DhtOpHash::from_raw_36(vec![0x99; 36]))
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn limbo_op_hashes_requiring_receipt_filters_on_flag() {
        let db = test_open_db(dht_db_id()).await.unwrap();

        let needs_action = seed_action_for_op(&db, 1).await;
        let needs = DhtOpHash::from_raw_36(vec![0x11; 36]);
        db.insert_limbo_chain_op(InsertLimboChainOp {
            op_hash: &needs,
            action_hash: &needs_action,
            op_type: 1,
            basis_hash: &sample_basis(1),
            storage_center_loc: 0,
            require_receipt: true,
            when_received: Timestamp::from_micros(1),
            serialized_size: 0,
        })
        .await
        .unwrap();

        let no_need_action = seed_action_for_op(&db, 2).await;
        db.insert_limbo_chain_op(InsertLimboChainOp {
            op_hash: &DhtOpHash::from_raw_36(vec![0x22; 36]),
            action_hash: &no_need_action,
            op_type: 1,
            basis_hash: &sample_basis(2),
            storage_center_loc: 0,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            serialized_size: 0,
        })
        .await
        .unwrap();

        assert_eq!(
            db.as_ref()
                .limbo_op_hashes_requiring_receipt()
                .await
                .unwrap(),
            vec![needs]
        );
    }

    #[tokio::test]
    async fn get_ops_at_basis_matches_basis_hash() {
        let db = test_open_db(dht_db_id()).await.unwrap();

        let action = seed_action_for_op(&db, 1).await;
        db.insert_chain_op(InsertChainOp {
            op_hash: &DhtOpHash::from_raw_36(vec![0x11; 36]),
            action_hash: &action,
            op_type: 1,
            basis_hash: &sample_basis(7),
            storage_center_loc: 0,
            validation_status: RecordValidity::Accepted,
            locally_validated: true,
            require_receipt: false,
            when_received: Timestamp::from_micros(1),
            when_integrated: Timestamp::from_micros(2),
            serialized_size: 0,
        })
        .await
        .unwrap();

        assert!(!db
            .as_ref()
            .get_ops_at_basis(&sample_basis(7))
            .await
            .unwrap()
            .is_empty());
        assert!(db
            .as_ref()
            .get_ops_at_basis(&sample_basis(8))
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn count_entries_counts_entry_rows() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        assert_eq!(db.as_ref().count_entries().await.unwrap(), 0);

        let (hash, entry) = sample_entry(7);
        db.insert_entry(&hash, &entry).await.unwrap();
        assert_eq!(db.as_ref().count_entries().await.unwrap(), 1);
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
            signature: &[1u8; 64],
            reason: None,
            storage_center_loc: 0,
            when_received: Timestamp::from_micros(3),
            when_integrated: Timestamp::from_micros(5),
            validation_status: 1,
            serialized_size: 64,
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
            Timestamp::from_micros(42),
        )
        .await
        .unwrap();

        let rows = db.as_ref().get_validation_receipts(op_hash).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].hash, receipt_hash.get_raw_36().to_vec());
        assert_eq!(rows[0].blob, vec![1u8; 16]);
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
            signature: &[0u8; 64],
            reason: None,
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
            signature: &[6u8; 64],
            reason: Some("invalid chain op"),
            storage_center_loc: 77,
            when_received: Timestamp::from_micros(100),
            serialized_size: 128,
        })
        .await
        .unwrap();

        // Only warrants with a terminal sys-validation status are promoted; the
        // status is carried into `WarrantOp.validation_status`.
        db.set_limbo_warrant_sys_validation_status(&hash, Some(1))
            .await
            .unwrap();

        let promoted = db
            .promote_limbo_warrant(&hash, Timestamp::from_micros(200))
            .await
            .unwrap();
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
        assert_eq!(row.author, author.get_raw_36().to_vec());
        assert_eq!(row.timestamp, 10);
        assert_eq!(row.warrantee, warrantee.get_raw_36().to_vec());
        assert_eq!(row.proof, vec![5u8; 64]);
        assert_eq!(row.signature, vec![6u8; 64]);
        assert_eq!(row.storage_center_loc, 77);
        assert_eq!(row.when_received, 100);
        // `when_integrated` is the promotion timestamp, not the limbo one.
        assert_eq!(row.when_integrated, 200);
        assert_eq!(row.serialized_size, 128);
        // The reason rides on the shared `Warrant` content row, so it
        // survives promotion unchanged.
        assert_eq!(row.reason.as_deref(), Some("invalid chain op"));
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

    /// Seed a self-authored countersigning-style record: a `Create` action plus
    /// one `ChainOp` per `op_byte` (each with a `ChainOpPublish` row carrying
    /// `withhold`), and the entry stored in `Entry` (public) or `PrivateEntry`
    /// (when `private_author` is `Some`). Returns the action hash and op hashes.
    async fn seed_countersigning_record(
        db: &crate::handles::DbWrite<Dht>,
        seed: u8,
        entry_hash: &EntryHash,
        op_specs: &[(u8, Option<bool>)],
        private_author: Option<&AgentPubKey>,
    ) -> (ActionHash, Vec<DhtOpHash>) {
        let author = AgentPubKey::from_raw_36(vec![seed; 36]);
        let action_hash = ActionHash::from_raw_36(vec![seed; 36]);
        let action = Action {
            header: ActionHeader {
                author: author.clone(),
                timestamp: Timestamp::from_micros(1_000_000 + seed as i64),
                action_seq: seed as u32,
                prev_action: Some(ActionHash::from_raw_36(vec![seed.wrapping_sub(1); 36])),
            },
            data: ActionData::Create(holochain_integrity_types::dht_v2::CreateData {
                entry_type: holochain_integrity_types::EntryType::AgentPubKey,
                entry_hash: entry_hash.clone(),
            }),
        };
        let hashed = HoloHashed::with_pre_hashed(action, action_hash.clone());
        let signed = SignedHashed::with_presigned(hashed, Signature([seed; 64]));
        db.insert_action(&signed, Some(RecordValidity::Accepted))
            .await
            .unwrap();

        let mut op_hashes = Vec::new();
        for (op_byte, withhold) in op_specs {
            let op_hash = DhtOpHash::from_raw_36(vec![*op_byte; 36]);
            db.insert_chain_op(InsertChainOp {
                op_hash: &op_hash,
                action_hash: &action_hash,
                op_type: 1,
                basis_hash: &sample_basis(seed),
                storage_center_loc: 0,
                validation_status: RecordValidity::Accepted,
                locally_validated: true,
                require_receipt: false,
                when_received: Timestamp::from_micros(1),
                when_integrated: Timestamp::from_micros(2),
                serialized_size: 0,
            })
            .await
            .unwrap();
            db.insert_chain_op_publish(&op_hash, None, None, *withhold)
                .await
                .unwrap();
            op_hashes.push(op_hash);
        }

        let entry = Entry::Agent(author.clone());
        match private_author {
            Some(a) => db
                .insert_private_entry(entry_hash, a, &entry)
                .await
                .unwrap(),
            None => db.insert_entry(entry_hash, &entry).await.unwrap(),
        }

        (action_hash, op_hashes)
    }

    #[tokio::test]
    async fn remove_countersigning_session_deletes_withheld_public_entry() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let entry_hash = EntryHash::from_raw_36(vec![0x77; 36]);
        // Two withheld self-authored ops for one action (e.g. StoreRecord +
        // StoreEntry) plus the public entry.
        let (action_hash, op_hashes) = seed_countersigning_record(
            &db,
            0x30,
            &entry_hash,
            &[(0xA1, Some(true)), (0xA2, Some(true))],
            None,
        )
        .await;

        let outcome = db
            .remove_countersigning_session(&action_hash, &entry_hash)
            .await
            .unwrap();
        assert_eq!(outcome, RemoveCountersigningSessionOutcome::Removed);

        // Action, both ops, both publish rows, and the public entry are gone.
        assert!(db.as_ref().get_action(action_hash).await.unwrap().is_none());
        for op_hash in op_hashes {
            assert!(db
                .as_ref()
                .get_chain_op(op_hash.clone())
                .await
                .unwrap()
                .is_none());
            assert!(db
                .as_ref()
                .get_chain_op_publish(op_hash)
                .await
                .unwrap()
                .is_none());
        }
        assert!(db
            .as_ref()
            .get_entry(entry_hash, None)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn remove_countersigning_session_deletes_private_entry() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let author = AgentPubKey::from_raw_36(vec![0x31; 36]);
        let entry_hash = EntryHash::from_raw_36(vec![0x78; 36]);
        let (action_hash, _) = seed_countersigning_record(
            &db,
            0x31,
            &entry_hash,
            &[(0xB1, Some(true))],
            Some(&author),
        )
        .await;

        // The private entry is present before removal.
        assert!(db
            .as_ref()
            .get_entry(entry_hash.clone(), Some(&author))
            .await
            .unwrap()
            .is_some());

        let outcome = db
            .remove_countersigning_session(&action_hash, &entry_hash)
            .await
            .unwrap();
        assert_eq!(outcome, RemoveCountersigningSessionOutcome::Removed);

        assert!(db.as_ref().get_action(action_hash).await.unwrap().is_none());
        assert!(db
            .as_ref()
            .get_entry(entry_hash, Some(&author))
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn remove_countersigning_session_refuses_when_published() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let entry_hash = EntryHash::from_raw_36(vec![0x79; 36]);
        // One withheld op and one published op (withhold cleared) for the same
        // action: the published op must block removal.
        let (action_hash, op_hashes) = seed_countersigning_record(
            &db,
            0x32,
            &entry_hash,
            &[(0xC1, Some(true)), (0xC2, None)],
            None,
        )
        .await;

        let outcome = db
            .remove_countersigning_session(&action_hash, &entry_hash)
            .await
            .unwrap();
        assert_eq!(
            outcome,
            RemoveCountersigningSessionOutcome::AlreadyPublished
        );

        // Nothing was deleted.
        assert!(db.as_ref().get_action(action_hash).await.unwrap().is_some());
        for op_hash in op_hashes {
            assert!(db.as_ref().get_chain_op(op_hash).await.unwrap().is_some());
        }
        assert!(db
            .as_ref()
            .get_entry(entry_hash, None)
            .await
            .unwrap()
            .is_some());
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
            require_receipt: false,
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

    // Build a unique action hash from the author's first byte and the seq, so
    // that two inserts for different (author, seq) pairs never collide on the
    // primary key.
    async fn insert_test_action(
        db: &crate::handles::DbWrite<Dht>,
        author: &AgentPubKey,
        seq: u32,
    ) -> (ActionHash, Timestamp) {
        let mut hash_bytes = vec![0u8; 36];
        hash_bytes[0] = author.get_raw_36()[0];
        hash_bytes[1] = (seq & 0xff) as u8;
        hash_bytes[2] = ((seq >> 8) & 0xff) as u8;
        let hash = ActionHash::from_raw_36(hash_bytes);
        let ts = Timestamp::from_micros(1_000_000 + seq as i64);
        let action = Action {
            header: ActionHeader {
                author: author.clone(),
                timestamp: ts,
                action_seq: seq,
                prev_action: None,
            },
            data: ActionData::InitZomesComplete(InitZomesCompleteData {}),
        };
        let hashed = HoloHashed::with_pre_hashed(action, hash.clone());
        let signed = SignedHashed::with_presigned(hashed, Signature([0u8; 64]));
        // The head lookup reads the `Action` row's own `record_validity`, so a
        // self-authored (`Accepted`) action is recognised as the head without
        // any accompanying op.
        db.insert_action(&signed, Some(RecordValidity::Accepted))
            .await
            .unwrap();
        (hash, ts)
    }

    #[tokio::test]
    async fn chain_head_for_author_returns_max_seq() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let author = AgentPubKey::from_raw_36(vec![0x01; 36]);
        let other = AgentPubKey::from_raw_36(vec![0x02; 36]);

        insert_test_action(&db, &author, 0).await;
        let head = insert_test_action(&db, &author, 1).await;
        // Other author at a higher seq — must not affect `author`'s head.
        insert_test_action(&db, &other, 5).await;

        let got = db.as_ref().chain_head_for_author(&author).await.unwrap();
        assert_eq!(got, Some((head.0, 1, head.1)));
    }

    #[tokio::test]
    async fn chain_head_for_author_empty_chain_is_none() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let author = AgentPubKey::from_raw_36(vec![0x03; 36]);
        assert_eq!(
            db.as_ref().chain_head_for_author(&author).await.unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn set_chain_op_receipts_complete_round_trip() {
        let db = test_open_db(dht_db_id()).await.unwrap();

        let action = sample_action(7);
        db.insert_action(&action, Some(RecordValidity::Accepted))
            .await
            .unwrap();

        let op_hash = DhtOpHash::from_raw_36(vec![9u8; 36]);
        let basis_hash = AnyLinkableHash::from_raw_36_and_type(
            vec![9u8; 36],
            holo_hash::hash_type::AnyLinkable::Action,
        );

        db.insert_chain_op(InsertChainOp {
            op_hash: &op_hash,
            action_hash: action.as_hash(),
            op_type: 1,
            basis_hash: &basis_hash,
            storage_center_loc: 0,
            validation_status: RecordValidity::Accepted,
            locally_validated: true,
            require_receipt: false,
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

    #[tokio::test]
    async fn live_scheduled_functions_predicate() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let alice = AgentPubKey::from_raw_36(vec![0x01; 36]);
        let bob = AgentPubKey::from_raw_36(vec![0x02; 36]);
        let now = Timestamp::from_micros(500);
        let payload = b"schedule-blob";

        // Alice: ephemeral, live (start_at=200 <= now=500 <= end_at=MAX).
        db.upsert_scheduled_function(InsertScheduledFunction {
            author: &alice,
            zome_name: "z",
            scheduled_fn: "ephemeral_live",
            maybe_schedule: payload,
            start_at: Timestamp::from_micros(200),
            end_at: Timestamp::from_micros(i64::MAX),
            ephemeral: true,
        })
        .await
        .unwrap();

        // Alice: persisted, future (start_at=700 > now=500 → not live).
        db.upsert_scheduled_function(InsertScheduledFunction {
            author: &alice,
            zome_name: "z",
            scheduled_fn: "persisted_future",
            maybe_schedule: payload,
            start_at: Timestamp::from_micros(700),
            end_at: Timestamp::from_micros(900),
            ephemeral: false,
        })
        .await
        .unwrap();

        // Alice: ephemeral, already past end_at (now=500 > end_at=400 → not live).
        db.upsert_scheduled_function(InsertScheduledFunction {
            author: &alice,
            zome_name: "z",
            scheduled_fn: "past",
            maybe_schedule: payload,
            start_at: Timestamp::from_micros(100),
            end_at: Timestamp::from_micros(400),
            ephemeral: true,
        })
        .await
        .unwrap();

        // Bob: ephemeral, live — must NOT be returned for alice.
        db.upsert_scheduled_function(InsertScheduledFunction {
            author: &bob,
            zome_name: "z",
            scheduled_fn: "ephemeral_live",
            maybe_schedule: payload,
            start_at: Timestamp::from_micros(200),
            end_at: Timestamp::from_micros(i64::MAX),
            ephemeral: true,
        })
        .await
        .unwrap();

        let live = db
            .as_ref()
            .get_live_scheduled_functions(&alice, now)
            .await
            .unwrap();

        assert_eq!(live.len(), 1, "expected exactly one live fn for alice");
        assert_eq!(live[0].0, "z");
        assert_eq!(live[0].1, "ephemeral_live");
        assert!(live[0].3, "expected ephemeral=true for the live fn");
    }
}
