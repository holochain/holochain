//! Cache operations on the per-DNA DHT store.
//!
//! Chain ops fetched from peer authorities are inserted with
//! `locally_validated = false`, bypassing limbo. Warrants are always routed
//! through limbo (`LimboWarrantOp`) so the local conductor can validate them
//! regardless of arc coverage.

use holo_hash::HasHash;
use holochain_data::dht::{InsertChainOp, InsertLimboWarrant};
use holochain_data::kind::Dht;
use holochain_data::DbWrite;
use holochain_types::prelude::Timestamp;
use holochain_types::warrant::WarrantOp;
use holochain_types::wire_ops::RenderedOps;
use holochain_zome_types::action::RecordValidity;

use super::action_indexes::insert_action_indexes;
use super::DhtStore;
use crate::mutations::{StateMutationError, StateMutationResult};

impl DhtStore<DbWrite<Dht>> {
    /// Insert a batch of chain ops into the DHT store cache.
    ///
    /// Each op is recorded with `locally_validated = false`,
    /// `validation_status = Accepted`, and `when_received` /
    /// `when_integrated` set to the current time. The integration indices
    /// (`Link`, `DeletedLink`, `UpdatedRecord`, `DeletedRecord`) are
    /// populated based on the action variant.
    ///
    /// The shared entry, if present, is inserted once for the whole
    /// `RenderedOps`. Any `ops.warrant` is ignored; warrants are inserted
    /// via [`Self::stage_warrants_for_validation`].
    pub async fn cache_chain_ops(&self, ops: &RenderedOps) -> StateMutationResult<()> {
        if ops.ops.is_empty() && ops.entry.is_none() {
            return Ok(());
        }

        let mut tx = self.db().begin().await.map_err(StateMutationError::from)?;
        let now = Timestamp::now();

        if let Some(entry) = ops.entry.as_ref() {
            tx.insert_entry(entry.as_hash(), entry.as_content())
                .await
                .map_err(StateMutationError::from)?;
        }

        for op in &ops.ops {
            tx.insert_action(&op.action, None)
                .await
                .map_err(StateMutationError::from)?;

            insert_action_indexes(&mut tx, op.action.as_hash(), &op.action.hashed.content.data)
                .await?;

            tx.insert_chain_op(InsertChainOp {
                op_hash: &op.op_hash,
                action_hash: op.action.as_hash(),
                op_type: i64::from(op.op_type),
                basis_hash: &op.basis_hash,
                storage_center_loc: op.storage_center_loc,
                validation_status: RecordValidity::Accepted,
                locally_validated: false,
                require_receipt: false,
                when_received: now,
                when_integrated: now,
                serialized_size: 0,
            })
            .await
            .map_err(StateMutationError::from)?;
        }

        tx.commit().await.map_err(StateMutationError::from)?;
        Ok(())
    }

    /// Insert cached warrants into `Warrant` + `LimboWarrantOp`.
    ///
    /// Warrants must be locally validated regardless of arc coverage, so they
    /// are routed through limbo rather than inserted directly.
    pub async fn stage_warrants_for_validation(
        &self,
        warrants: Vec<WarrantOp>,
    ) -> StateMutationResult<()> {
        if warrants.is_empty() {
            return Ok(());
        }

        let mut tx = self.db().begin().await.map_err(StateMutationError::from)?;
        let now = Timestamp::now();

        for warrant_op in warrants {
            let op_hash = holo_hash::DhtOpHash::with_data_sync(&warrant_op);
            let proof_bytes = holochain_serialized_bytes::encode(&warrant_op.proof)
                .map_err(StateMutationError::from)?;
            let signature_bytes = warrant_op.signature().0;
            let storage_center_loc = warrant_op.warrantee.get_loc();
            let serialized_size = holochain_serialized_bytes::encode(&warrant_op)
                .map_err(StateMutationError::from)?
                .len() as u32;

            tx.insert_limbo_warrant(InsertLimboWarrant {
                hash: &op_hash,
                author: &warrant_op.author,
                timestamp: warrant_op.timestamp,
                warrantee: &warrant_op.warrantee,
                proof: &proof_bytes,
                signature: &signature_bytes,
                reason: warrant_op.proof.reason(),
                storage_center_loc,
                when_received: now,
                serialized_size,
            })
            .await
            .map_err(StateMutationError::from)?;
        }

        tx.commit().await.map_err(StateMutationError::from)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use holo_hash::{ActionHash, AgentPubKey, AnyLinkableHash, DnaHash, EntryHash};
    use holochain_serialized_bytes::UnsafeBytes;
    use holochain_types::prelude::{AppEntryBytes, Entry, EntryHashed, Signature};
    use holochain_types::warrant::WarrantOp;
    use holochain_types::wire_ops::{RenderedOp, RenderedOps};
    use holochain_zome_types::action::{
        Action, ActionData, ActionHeader, CreateData, CreateLinkData, DeleteData, DeleteLinkData,
        UpdateData,
    };
    use holochain_zome_types::entry_def::EntryVisibility;
    use holochain_zome_types::op::ChainOpType;
    use holochain_zome_types::prelude::EntryType;
    use holochain_zome_types::prelude::{
        AppEntryDef, ChainIntegrityWarrant, SignedWarrant, Warrant, WarrantProof,
    };
    use std::sync::Arc;

    fn dht_id() -> Dht {
        Dht::new(Arc::new(DnaHash::from_raw_36(vec![0u8; 36])))
    }

    /// Build an [`Action`] with the given header fields and per-variant data.
    fn mk_action(
        author: AgentPubKey,
        seq: u32,
        prev: ActionHash,
        ts: i64,
        data: ActionData,
    ) -> Action {
        Action {
            header: ActionHeader {
                author,
                timestamp: Timestamp::from_micros(ts),
                action_seq: seq,
                prev_action: Some(prev),
            },
            data,
        }
    }

    fn app_public_entry_type() -> EntryType {
        EntryType::App(AppEntryDef::new(
            0.into(),
            0.into(),
            EntryVisibility::Public,
        ))
    }

    /// Build a single-op `RenderedOps` for a `StoreRecord(Create)` chain op
    /// carrying a public entry.
    fn build_rendered_store_record(seed: u8) -> RenderedOps {
        let author = AgentPubKey::from_raw_36(vec![seed; 36]);
        let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
        let entry = Entry::App(AppEntryBytes(
            holochain_serialized_bytes::SerializedBytes::from(UnsafeBytes::from(vec![seed; 8])),
        ));
        let sig = Signature::from([seed; 64]);
        let action = mk_action(
            author,
            1,
            ActionHash::from_raw_36(vec![seed.wrapping_add(200); 36]),
            seed as i64 * 1000,
            ActionData::Create(CreateData {
                entry_type: app_public_entry_type(),
                entry_hash: entry_hash.clone(),
            }),
        );
        let entry_hashed = EntryHashed::with_pre_hashed(entry.clone(), entry_hash);

        let rendered = RenderedOp::new(action, sig, None, ChainOpType::StoreRecord)
            .expect("rendered op build");

        RenderedOps {
            entry: Some(entry_hashed),
            ops: vec![rendered],
            warrant: None,
        }
    }

    /// Build a single-op `RenderedOps` for a CreateLink chain op
    /// (`RegisterAddLink`). No entry.
    fn build_rendered_create_link(seed: u8) -> RenderedOps {
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
        let action = mk_action(
            author,
            2,
            ActionHash::from_raw_36(vec![seed.wrapping_add(70); 36]),
            seed as i64 * 1000,
            ActionData::CreateLink(CreateLinkData {
                base_address: base,
                target_address: target,
                zome_index: 0.into(),
                link_type: 0.into(),
                tag: holochain_zome_types::link::LinkTag(vec![1, 2, 3]),
            }),
        );

        let rendered = RenderedOp::new(action, sig, None, ChainOpType::RegisterAddLink)
            .expect("rendered op build");
        RenderedOps {
            entry: None,
            ops: vec![rendered],
            warrant: None,
        }
    }

    /// Build a single-op `RenderedOps` for a DeleteLink chain op
    /// (`RegisterRemoveLink`). No entry.
    fn build_rendered_delete_link(seed: u8) -> RenderedOps {
        let author = AgentPubKey::from_raw_36(vec![seed; 36]);
        let base = AnyLinkableHash::from_raw_36_and_type(
            vec![seed.wrapping_add(40); 36],
            holo_hash::hash_type::AnyLinkable::Entry,
        );
        let link_add = ActionHash::from_raw_36(vec![seed.wrapping_add(80); 36]);
        let sig = Signature::from([seed; 64]);
        let action = mk_action(
            author,
            3,
            ActionHash::from_raw_36(vec![seed.wrapping_add(90); 36]),
            seed as i64 * 1000,
            ActionData::DeleteLink(DeleteLinkData {
                base_address: base,
                link_add_address: link_add,
            }),
        );
        let rendered = RenderedOp::new(action, sig, None, ChainOpType::RegisterRemoveLink)
            .expect("rendered op build");
        RenderedOps {
            entry: None,
            ops: vec![rendered],
            warrant: None,
        }
    }

    /// Build a single-op `RenderedOps` for an Update chain op
    /// (`RegisterUpdatedRecord`).
    fn build_rendered_update(seed: u8) -> RenderedOps {
        let author = AgentPubKey::from_raw_36(vec![seed; 36]);
        let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
        let original_action = ActionHash::from_raw_36(vec![seed.wrapping_add(20); 36]);
        let original_entry = EntryHash::from_raw_36(vec![seed.wrapping_add(30); 36]);
        let entry = Entry::App(AppEntryBytes(
            holochain_serialized_bytes::SerializedBytes::from(UnsafeBytes::from(vec![seed; 8])),
        ));
        let sig = Signature::from([seed; 64]);
        let action = mk_action(
            author,
            2,
            ActionHash::from_raw_36(vec![seed.wrapping_add(70); 36]),
            seed as i64 * 1000,
            ActionData::Update(UpdateData {
                original_action_address: original_action,
                original_entry_address: original_entry,
                entry_type: app_public_entry_type(),
                entry_hash: entry_hash.clone(),
            }),
        );
        let entry_hashed = EntryHashed::with_pre_hashed(entry.clone(), entry_hash);
        let rendered = RenderedOp::new(action, sig, None, ChainOpType::RegisterUpdatedRecord)
            .expect("rendered op build");
        RenderedOps {
            entry: Some(entry_hashed),
            ops: vec![rendered],
            warrant: None,
        }
    }

    /// Build a single-op `RenderedOps` for a Delete chain op
    /// (`RegisterDeletedBy`).
    fn build_rendered_delete(seed: u8) -> RenderedOps {
        let author = AgentPubKey::from_raw_36(vec![seed; 36]);
        let deletes_address = ActionHash::from_raw_36(vec![seed.wrapping_add(20); 36]);
        let deletes_entry = EntryHash::from_raw_36(vec![seed.wrapping_add(30); 36]);
        let sig = Signature::from([seed; 64]);
        let action = mk_action(
            author,
            3,
            ActionHash::from_raw_36(vec![seed.wrapping_add(70); 36]),
            seed as i64 * 1000,
            ActionData::Delete(DeleteData {
                deletes_address,
                deletes_entry_address: deletes_entry,
            }),
        );
        let rendered = RenderedOp::new(action, sig, None, ChainOpType::RegisterDeletedBy)
            .expect("rendered op build");
        RenderedOps {
            entry: None,
            ops: vec![rendered],
            warrant: None,
        }
    }

    /// Build a `RegisterAgentActivity` chain op as `RenderedOps`.
    fn build_rendered_activity(seed: u8) -> RenderedOps {
        let author = AgentPubKey::from_raw_36(vec![seed; 36]);
        let sig = Signature::from([seed; 64]);
        let action = mk_action(
            author,
            1,
            ActionHash::from_raw_36(vec![seed.wrapping_add(200); 36]),
            seed as i64 * 1000,
            ActionData::Create(CreateData {
                entry_type: app_public_entry_type(),
                entry_hash: EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]),
            }),
        );
        let rendered = RenderedOp::new(action, sig, None, ChainOpType::RegisterAgentActivity)
            .expect("rendered op build");
        RenderedOps {
            entry: None,
            ops: vec![rendered],
            warrant: None,
        }
    }

    /// Build a `WarrantOp`.
    fn build_warrant_op(seed: u8) -> WarrantOp {
        let action_author = AgentPubKey::from_raw_36(vec![seed; 36]);
        let warrantee = AgentPubKey::from_raw_36(vec![seed.wrapping_add(50); 36]);
        let action_hash = ActionHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
        let warrant = SignedWarrant::new(
            Warrant::new(
                WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
                    action_author,
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
        WarrantOp::from(warrant)
    }

    fn op_hash_of(rendered: &RenderedOps) -> holo_hash::DhtOpHash {
        rendered.ops[0].op_hash.clone()
    }

    #[tokio::test]
    async fn cache_chain_ops_inserts_action_and_entry() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let rendered = build_rendered_store_record(1);
        let entry_hash = rendered.entry.as_ref().unwrap().as_hash().clone();
        let action_hash = rendered.ops[0].action.as_hash().clone();
        let op_hash = op_hash_of(&rendered);

        store.cache_chain_ops(&rendered).await.unwrap();

        let action = store.db().as_ref().get_action(action_hash).await.unwrap();
        assert!(action.is_some(), "Action row missing after cache record");

        let entry = store
            .db()
            .as_ref()
            .get_entry(entry_hash, None)
            .await
            .unwrap();
        assert!(entry.is_some(), "Entry row missing after cache record");

        let op = store.db().as_ref().get_chain_op(op_hash).await.unwrap();
        assert!(op.is_some(), "ChainOp row missing after cache record");
    }

    #[tokio::test]
    async fn cache_chain_ops_sets_locally_validated_false() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let rendered = build_rendered_store_record(2);
        let op_hash = op_hash_of(&rendered);

        store.cache_chain_ops(&rendered).await.unwrap();

        let row = store
            .db()
            .as_ref()
            .get_chain_op(op_hash)
            .await
            .unwrap()
            .expect("ChainOp row missing");
        assert_eq!(
            row.locally_validated, 0,
            "cached chain op should have locally_validated = 0"
        );
        assert_eq!(
            row.validation_status,
            i64::from(RecordValidity::Accepted),
            "cached chain op should be Accepted"
        );
    }

    #[tokio::test]
    async fn cache_chain_ops_populates_link_index() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let rendered = build_rendered_create_link(3);
        let action_hash = rendered.ops[0].action.as_hash().clone();
        let base = match &rendered.ops[0].action.action().data {
            ActionData::CreateLink(a) => a.base_address.clone(),
            _ => panic!("expected CreateLink"),
        };

        store.cache_chain_ops(&rendered).await.unwrap();

        let rows = store.db().as_ref().get_links_by_base(base).await.unwrap();
        assert_eq!(rows.len(), 1, "expected one link row for cached CreateLink");
        assert_eq!(rows[0].action_hash, action_hash.get_raw_36().to_vec());
    }

    #[tokio::test]
    async fn cache_chain_ops_populates_deleted_link_index() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let rendered = build_rendered_delete_link(4);
        let create_link_hash = match &rendered.ops[0].action.action().data {
            ActionData::DeleteLink(a) => a.link_add_address.clone(),
            _ => panic!("expected DeleteLink"),
        };

        store.cache_chain_ops(&rendered).await.unwrap();

        let rows = store
            .db()
            .as_ref()
            .get_deleted_links(create_link_hash)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1, "expected one deleted_link row");
    }

    #[tokio::test]
    async fn cache_chain_ops_populates_updated_record_index() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let rendered = build_rendered_update(5);
        let original_action = match &rendered.ops[0].action.action().data {
            ActionData::Update(a) => a.original_action_address.clone(),
            _ => panic!("expected Update"),
        };

        store.cache_chain_ops(&rendered).await.unwrap();

        let rows = store
            .db()
            .as_ref()
            .get_updated_records(original_action)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1, "expected one updated_record row");
    }

    #[tokio::test]
    async fn cache_chain_ops_populates_deleted_record_index() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let rendered = build_rendered_delete(6);
        let deletes_address = match &rendered.ops[0].action.action().data {
            ActionData::Delete(a) => a.deletes_address.clone(),
            _ => panic!("expected Delete"),
        };

        store.cache_chain_ops(&rendered).await.unwrap();

        let rows = store
            .db()
            .as_ref()
            .get_deleted_records(deletes_address)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1, "expected one deleted_record row");
    }

    #[tokio::test]
    async fn cache_chain_ops_inserts_agent_activity() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let rendered = build_rendered_activity(7);
        let op_hash = op_hash_of(&rendered);

        store.cache_chain_ops(&rendered).await.unwrap();

        let row = store
            .db()
            .as_ref()
            .get_chain_op(op_hash)
            .await
            .unwrap()
            .expect("ChainOp row missing after activity cache record");
        assert_eq!(row.locally_validated, 0);
        assert_eq!(row.validation_status, i64::from(RecordValidity::Accepted));
    }

    #[tokio::test]
    async fn stage_warrants_for_validation_enters_limbo() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let warrant_op = build_warrant_op(8);
        let op_hash = holo_hash::DhtOpHash::with_data_sync(&warrant_op);

        store
            .stage_warrants_for_validation(vec![warrant_op])
            .await
            .unwrap();

        let row = store
            .db()
            .as_ref()
            .get_limbo_warrant(op_hash)
            .await
            .unwrap();
        assert!(row.is_some(), "LimboWarrant row missing for cached warrant");
    }
}
