//! Cache mirror methods on `DhtStore<DbWrite<Dht>>`.
//!
//! Records cached data fetched from peer authorities. Chain ops bypass limbo
//! (`locally_validated = false`); warrants always go to `LimboWarrant` for
//! local validation regardless of arc.

use holo_hash::{AnyDhtHash, HasHash};
use holochain_data::dht::{
    InsertChainOp, InsertDeletedLink, InsertDeletedRecord, InsertLimboWarrant, InsertLink,
    InsertUpdatedRecord,
};
use holochain_data::kind::Dht;
use holochain_data::DbWrite;
use holochain_types::dht_op::{DhtOp, DhtOpHashed, RenderedOps};
use holochain_types::prelude::Timestamp;
use holochain_zome_types::action::Action;
use holochain_zome_types::dht_v2::OpValidity;

use super::DhtStore;
use crate::mutations::{StateMutationError, StateMutationResult};

impl DhtStore<DbWrite<Dht>> {
    /// Insert cached chain ops fetched from peer authorities directly into
    /// `ChainOp` (bypassing limbo).
    ///
    /// Each op is recorded with `locally_validated = false`,
    /// `validation_status = Accepted`, `require_receipt = false`, and
    /// `when_received` / `when_integrated` set to the current time. The
    /// integration indices (`Link`, `DeletedLink`, `UpdatedRecord`,
    /// `DeletedRecord`) are populated based on the action variant.
    ///
    /// The entry, if any, is inserted once for the whole `RenderedOps`.
    /// Any `ops.warrant` is ignored: cascade routes cached warrants through
    /// [`Self::record_incoming_cached_warrants`] instead.
    pub async fn record_cached_chain_ops(&self, ops: &RenderedOps) -> StateMutationResult<()> {
        if ops.ops.is_empty() && ops.entry.is_none() {
            return Ok(());
        }

        let mut tx = self.db().begin().await.map_err(StateMutationError::from)?;
        let now = Timestamp::now();

        // Insert the shared entry, if present.
        if let Some(entry) = ops.entry.as_ref() {
            tx.insert_entry(entry.as_hash(), entry.as_content())
                .await
                .map_err(StateMutationError::from)?;
        }

        for op in &ops.ops {
            // Convert the legacy SignedActionHashed to the new v2 form.
            let new_sah = crate::source_chain::legacy_to_dht_v2_signed_action(&op.action);
            tx.insert_action(&new_sah, None)
                .await
                .map_err(StateMutationError::from)?;

            // Populate integration index tables based on the action variant.
            // Use the legacy Action for dispatch since `RenderedOp.action`
            // carries the legacy form.
            match op.action.action() {
                Action::CreateLink(a) => {
                    tx.insert_link_index(InsertLink {
                        action_hash: new_sah.as_hash(),
                        base_hash: &a.base_address,
                        zome_index: a.zome_index.0,
                        link_type: a.link_type.0,
                        tag: Some(a.tag.0.as_slice()),
                    })
                    .await
                    .map_err(StateMutationError::from)?;
                }
                Action::DeleteLink(a) => {
                    tx.insert_deleted_link_index(InsertDeletedLink {
                        action_hash: new_sah.as_hash(),
                        create_link_hash: &a.link_add_address,
                    })
                    .await
                    .map_err(StateMutationError::from)?;
                }
                Action::Update(a) => {
                    tx.insert_updated_record_index(InsertUpdatedRecord {
                        action_hash: new_sah.as_hash(),
                        original_action_hash: &a.original_action_address,
                        original_entry_hash: &a.original_entry_address,
                    })
                    .await
                    .map_err(StateMutationError::from)?;
                }
                Action::Delete(a) => {
                    tx.insert_deleted_record_index(InsertDeletedRecord {
                        action_hash: new_sah.as_hash(),
                        deletes_action_hash: &a.deletes_address,
                        deletes_entry_hash: &a.deletes_entry_address,
                    })
                    .await
                    .map_err(StateMutationError::from)?;
                }
                _ => {}
            }

            // Insert the cached chain op row. Basis is taken from `op_light`.
            let linkable_basis = op.op_light.dht_basis();
            let storage_center_loc = linkable_basis.get_loc();
            let basis_hash: AnyDhtHash = AnyDhtHash::try_from(linkable_basis).map_err(|e| {
                StateMutationError::Other(format!(
                    "cannot convert cached op basis to AnyDhtHash: {e:?}"
                ))
            })?;

            let op_type = match op.op_light.get_type() {
                holochain_types::dht_op::DhtOpType::Chain(t) => i64::from(t),
                holochain_types::dht_op::DhtOpType::Warrant(_) => {
                    // RenderedOps.ops should only ever contain chain ops; the
                    // warrant lives on the parent struct.
                    return Err(StateMutationError::Other(
                        "RenderedOp had a Warrant op_light; expected Chain".into(),
                    ));
                }
            };

            tx.insert_chain_op(InsertChainOp {
                op_hash: &op.op_hash,
                action_hash: new_sah.as_hash(),
                op_type,
                basis_hash: &basis_hash,
                storage_center_loc,
                validation_status: OpValidity::Accepted,
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

    /// Insert cached `RegisterAgentActivity` ops directly into `ChainOp`
    /// (bypassing limbo).
    ///
    /// Each op is recorded with `locally_validated = false`,
    /// `validation_status = Accepted`, `require_receipt = false`, and
    /// `when_received` / `when_integrated` set to the current time.
    ///
    /// Any non-chain op in the input is logged and skipped defensively;
    /// callers should route warrants through
    /// [`Self::record_incoming_cached_warrants`].
    pub async fn record_cached_activity_ops(
        &self,
        ops: Vec<DhtOpHashed>,
    ) -> StateMutationResult<()> {
        if ops.is_empty() {
            return Ok(());
        }

        let mut tx = self.db().begin().await.map_err(StateMutationError::from)?;
        let now = Timestamp::now();

        for op in ops {
            let op_hash = op.as_hash().clone();
            let chain_op = match op.into_inner().0 {
                DhtOp::ChainOp(c) => c,
                DhtOp::WarrantOp(_) => {
                    tracing::warn!("record_cached_activity_ops got a non-chain DhtOp; skipping");
                    continue;
                }
            };

            // Convert the (legacy) signed action to the new v2 form.
            let signed_action = chain_op.signed_action();
            let action_hash = holo_hash::ActionHash::with_data_sync(signed_action.action());
            let sah = holochain_zome_types::record::SignedActionHashed::with_presigned(
                holo_hash::HoloHashed::with_pre_hashed(
                    signed_action.action().clone(),
                    action_hash.clone(),
                ),
                signed_action.signature().clone(),
            );
            let new_sah = crate::source_chain::legacy_to_dht_v2_signed_action(&sah);
            tx.insert_action(&new_sah, None)
                .await
                .map_err(StateMutationError::from)?;

            let linkable_basis = chain_op.dht_basis();
            let storage_center_loc = linkable_basis.get_loc();
            let basis_hash: AnyDhtHash = AnyDhtHash::try_from(linkable_basis).map_err(|e| {
                StateMutationError::Other(format!(
                    "cannot convert cached activity op basis to AnyDhtHash: {e:?}"
                ))
            })?;

            tx.insert_chain_op(InsertChainOp {
                op_hash: &op_hash,
                action_hash: &action_hash,
                op_type: i64::from(chain_op.get_type()),
                basis_hash: &basis_hash,
                storage_center_loc,
                validation_status: OpValidity::Accepted,
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

    /// Insert cached warrants into `LimboWarrant`.
    ///
    /// Warrants must always be locally validated regardless of arc, so cached
    /// warrants do not bypass limbo. Any non-warrant op in the input is
    /// logged and skipped defensively.
    pub async fn record_incoming_cached_warrants(
        &self,
        ops: Vec<DhtOpHashed>,
    ) -> StateMutationResult<()> {
        if ops.is_empty() {
            return Ok(());
        }

        let mut tx = self.db().begin().await.map_err(StateMutationError::from)?;
        let now = Timestamp::now();

        for op in ops {
            let op_hash = op.as_hash().clone();
            let warrant_op = match op.into_inner().0 {
                DhtOp::WarrantOp(w) => w,
                DhtOp::ChainOp(_) => {
                    tracing::warn!(
                        "record_incoming_cached_warrants got a non-warrant DhtOp; skipping"
                    );
                    continue;
                }
            };

            let proof_bytes = holochain_serialized_bytes::encode(&warrant_op.proof)
                .map_err(StateMutationError::from)?;
            let storage_center_loc = warrant_op.warrantee.get_loc();

            tx.insert_limbo_warrant(InsertLimboWarrant {
                hash: &op_hash,
                author: &warrant_op.author,
                timestamp: warrant_op.timestamp,
                warrantee: &warrant_op.warrantee,
                proof: &proof_bytes,
                storage_center_loc,
                when_received: now,
                serialized_size: 0,
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
    use holochain_types::dht_op::{ChainOp, DhtOp, DhtOpHashed, RenderedOp, RenderedOps};
    use holochain_types::prelude::{AppEntryBytes, Entry, EntryHashed, Signature};
    use holochain_types::warrant::WarrantOp;
    use holochain_zome_types::action::{
        Action, Create, CreateLink, Delete, DeleteLink, EntryType, Update,
    };
    use holochain_zome_types::entry_def::EntryVisibility;
    use holochain_zome_types::op::ChainOpType;
    use holochain_zome_types::prelude::{
        AppEntryDef, ChainIntegrityWarrant, SignedWarrant, Warrant, WarrantProof,
    };
    use std::sync::Arc;

    fn dht_id() -> Dht {
        Dht::new(Arc::new(DnaHash::from_raw_36(vec![0u8; 36])))
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
        let action = Action::CreateLink(CreateLink {
            author,
            timestamp: Timestamp::from_micros(seed as i64 * 1000),
            action_seq: 2,
            prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(70); 36]),
            base_address: base,
            target_address: target,
            zome_index: 0.into(),
            link_type: 0.into(),
            tag: holochain_zome_types::link::LinkTag(vec![1, 2, 3]),
            weight: Default::default(),
        });

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
        let action = Action::DeleteLink(DeleteLink {
            author,
            timestamp: Timestamp::from_micros(seed as i64 * 1000),
            action_seq: 3,
            prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(90); 36]),
            base_address: base,
            link_add_address: link_add,
        });
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
        let action = Action::Update(Update {
            author,
            timestamp: Timestamp::from_micros(seed as i64 * 1000),
            action_seq: 2,
            prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(70); 36]),
            original_action_address: original_action,
            original_entry_address: original_entry,
            entry_type: EntryType::App(AppEntryDef::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            )),
            entry_hash: entry_hash.clone(),
            weight: Default::default(),
        });
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
        let action = Action::Delete(Delete {
            author,
            timestamp: Timestamp::from_micros(seed as i64 * 1000),
            action_seq: 3,
            prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(70); 36]),
            deletes_address,
            deletes_entry_address: deletes_entry,
            weight: Default::default(),
        });
        let rendered = RenderedOp::new(action, sig, None, ChainOpType::RegisterDeletedBy)
            .expect("rendered op build");
        RenderedOps {
            entry: None,
            ops: vec![rendered],
            warrant: None,
        }
    }

    /// Build a `RegisterAgentActivity` chain op as `DhtOpHashed`.
    fn build_activity_op_hashed(seed: u8) -> DhtOpHashed {
        let author = AgentPubKey::from_raw_36(vec![seed; 36]);
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
            entry_hash: EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]),
            weight: Default::default(),
        });
        let op = DhtOp::ChainOp(Box::new(ChainOp::RegisterAgentActivity(sig, action)));
        DhtOpHashed::from_content_sync(op)
    }

    /// Build a `WarrantOp` as `DhtOpHashed`.
    fn build_warrant_op_hashed(seed: u8) -> DhtOpHashed {
        let action_author = AgentPubKey::from_raw_36(vec![seed; 36]);
        let warrantee = AgentPubKey::from_raw_36(vec![seed.wrapping_add(50); 36]);
        let action_hash = ActionHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
        let warrant = SignedWarrant::new(
            Warrant::new(
                WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
                    action_author,
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

    fn op_hash_of(rendered: &RenderedOps) -> holo_hash::DhtOpHash {
        rendered.ops[0].op_hash.clone()
    }

    #[tokio::test]
    async fn record_cached_chain_ops_inserts_action_and_entry() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let rendered = build_rendered_store_record(1);
        let entry_hash = rendered.entry.as_ref().unwrap().as_hash().clone();
        let action_hash = rendered.ops[0].action.as_hash().clone();
        let op_hash = op_hash_of(&rendered);

        store.record_cached_chain_ops(&rendered).await.unwrap();

        // Action present.
        let action = store.db().as_ref().get_action(action_hash).await.unwrap();
        assert!(action.is_some(), "Action row missing after cache record");

        // Entry present.
        let entry = store
            .db()
            .as_ref()
            .get_entry(entry_hash, None)
            .await
            .unwrap();
        assert!(entry.is_some(), "Entry row missing after cache record");

        // ChainOp row present.
        let op = store.db().as_ref().get_chain_op(op_hash).await.unwrap();
        assert!(op.is_some(), "ChainOp row missing after cache record");
    }

    #[tokio::test]
    async fn record_cached_chain_ops_sets_locally_validated_false() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let rendered = build_rendered_store_record(2);
        let op_hash = op_hash_of(&rendered);

        store.record_cached_chain_ops(&rendered).await.unwrap();

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
            i64::from(OpValidity::Accepted),
            "cached chain op should be Accepted"
        );
        assert_eq!(
            row.require_receipt, 0,
            "cached chain op should not require receipt"
        );
    }

    #[tokio::test]
    async fn record_cached_chain_ops_populates_link_index() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let rendered = build_rendered_create_link(3);
        let action_hash = rendered.ops[0].action.as_hash().clone();
        // Capture base hash from the action variant.
        let base = match rendered.ops[0].action.action() {
            Action::CreateLink(a) => a.base_address.clone(),
            _ => panic!("expected CreateLink"),
        };

        store.record_cached_chain_ops(&rendered).await.unwrap();

        let rows = store.db().as_ref().get_links_by_base(base).await.unwrap();
        assert_eq!(rows.len(), 1, "expected one link row for cached CreateLink");
        assert_eq!(rows[0].action_hash, action_hash.get_raw_36().to_vec());
    }

    #[tokio::test]
    async fn record_cached_chain_ops_populates_deleted_link_index() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let rendered = build_rendered_delete_link(4);
        let create_link_hash = match rendered.ops[0].action.action() {
            Action::DeleteLink(a) => a.link_add_address.clone(),
            _ => panic!("expected DeleteLink"),
        };

        store.record_cached_chain_ops(&rendered).await.unwrap();

        let rows = store
            .db()
            .as_ref()
            .get_deleted_links(create_link_hash)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1, "expected one deleted_link row");
    }

    #[tokio::test]
    async fn record_cached_chain_ops_populates_updated_record_index() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let rendered = build_rendered_update(5);
        let original_action = match rendered.ops[0].action.action() {
            Action::Update(a) => a.original_action_address.clone(),
            _ => panic!("expected Update"),
        };

        store.record_cached_chain_ops(&rendered).await.unwrap();

        let rows = store
            .db()
            .as_ref()
            .get_updated_records(original_action)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1, "expected one updated_record row");
    }

    #[tokio::test]
    async fn record_cached_chain_ops_populates_deleted_record_index() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let rendered = build_rendered_delete(6);
        let deletes_address = match rendered.ops[0].action.action() {
            Action::Delete(a) => a.deletes_address.clone(),
            _ => panic!("expected Delete"),
        };

        store.record_cached_chain_ops(&rendered).await.unwrap();

        let rows = store
            .db()
            .as_ref()
            .get_deleted_records(deletes_address)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1, "expected one deleted_record row");
    }

    #[tokio::test]
    async fn record_cached_activity_ops_inserts_chain_op() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let op = build_activity_op_hashed(7);
        let op_hash = op.as_hash().clone();

        store.record_cached_activity_ops(vec![op]).await.unwrap();

        let row = store
            .db()
            .as_ref()
            .get_chain_op(op_hash)
            .await
            .unwrap()
            .expect("ChainOp row missing after activity cache record");
        assert_eq!(row.locally_validated, 0);
        assert_eq!(row.validation_status, i64::from(OpValidity::Accepted));
    }

    #[tokio::test]
    async fn record_incoming_cached_warrants_enters_limbo() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let op = build_warrant_op_hashed(8);
        let op_hash = op.as_hash().clone();

        store
            .record_incoming_cached_warrants(vec![op])
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
