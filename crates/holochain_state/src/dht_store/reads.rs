//! Read operations on the per-DNA DHT store.
//!
//! Methods on [`DhtStoreRead`] expose domain-meaningful reads for the
//! holochain crate's workflows. They delegate to `holochain_data`'s
//! `DbRead<Dht>` primitives and return values in terms of the project's
//! existing domain types.

use super::{DhtStore, DhtStoreRead};
use crate::query::StateQueryResult;
use holo_hash::{DhtOpHash, HasHash};
use holochain_data::kind::Dht;
use holochain_data::{DbRead, DbWrite};
use holochain_types::dht_op::DhtOpHashed;

impl DhtStore<DbRead<Dht>> {
    /// Returns `true` if `hash` appears in any op-bearing DHT table
    /// (`ChainOp`, `LimboChainOp`, `Warrant`, `LimboWarrant`).
    pub async fn op_exists(&self, hash: &DhtOpHash) -> StateQueryResult<bool> {
        Ok(self.db().op_exists(hash).await?)
    }

    /// Drop any op whose hash is already recorded in the DHT store.
    /// Input order is preserved for surviving ops.
    pub async fn filter_existing_ops(
        &self,
        ops: Vec<DhtOpHashed>,
    ) -> StateQueryResult<Vec<DhtOpHashed>> {
        let hashes: Vec<DhtOpHash> = ops.iter().map(|o| o.as_hash().clone()).collect();
        let present = self.db().op_hashes_present(&hashes).await?;
        Ok(ops
            .into_iter()
            .zip(present)
            .filter_map(|(op, exists)| if exists { None } else { Some(op) })
            .collect())
    }

    /// Find an existing action that shares `prev_action` with the given
    /// `action` but has a different hash. Used by sys-validation to detect
    /// chain forks.
    ///
    /// Returns `Ok(None)` if `action` has no `prev_action`, or if no
    /// sibling exists. Returns `Err` with a descriptive message if a
    /// sibling action by a different author is found (cross-author
    /// `prev_action` collision).
    pub async fn find_fork_for_action(
        &self,
        action: &holochain_zome_types::action::Action,
    ) -> StateQueryResult<Option<holochain_types::prelude::SignedAction>> {
        use holochain_zome_types::dht_v2::to_legacy_signed_action;

        let Some(prev) = action.prev_action() else {
            return Ok(None);
        };
        let incoming_hash = holo_hash::ActionHash::with_data_sync(action);

        let siblings = self
            .db()
            .get_actions_by_prev_hash(prev, &incoming_hash)
            .await?;

        let incoming_author = action.author();
        if let Some(v2_sibling) = siblings.into_iter().next() {
            let legacy_sibling = to_legacy_signed_action(&v2_sibling).map_err(|e| {
                crate::query::StateQueryError::Other(format!(
                    "to_legacy_signed_action on sibling: {e}"
                ))
            })?;
            let existing_author = legacy_sibling.action().author();
            if existing_author != incoming_author {
                return Err(crate::query::StateQueryError::Other(format!(
                    "Cross-author prev_action collision: incoming author {incoming_author} \
                     differs from existing author {existing_author} for prev_action {prev:?}"
                )));
            }
            // Return the SignedAction (action + signature, no hash).
            let signature = legacy_sibling.signature().clone();
            let action = legacy_sibling.action().clone();
            Ok(Some(holochain_types::prelude::SignedAction::new(
                action, signature,
            )))
        } else {
            Ok(None)
        }
    }

    /// Return ops awaiting system validation, sorted across chain ops and
    /// warrants by `(sys_validation_attempts, when_received)`, up to `limit`
    /// rows total.
    pub async fn ops_pending_sys_validation(
        &self,
        limit: u32,
    ) -> StateQueryResult<Vec<DhtOpHashed>> {
        let db = self.db();
        let chain_rows = db.limbo_chain_ops_pending_sys_validation(limit).await?;
        let warrant_rows = db.limbo_warrants_pending_sys_validation(limit).await?;

        let mut out: Vec<(i64, i64, DhtOpHashed)> =
            Vec::with_capacity(chain_rows.len() + warrant_rows.len());

        for row in chain_rows {
            let attempts = row.sys_validation_attempts;
            let when_received = row.when_received;
            let op = chain_op_from_limbo_row(db, &row).await?;
            out.push((attempts, when_received, op));
        }
        for row in warrant_rows {
            let attempts = row.sys_validation_attempts;
            let when_received = row.when_received;
            let op = warrant_from_limbo_row(&row)?;
            out.push((attempts, when_received, op));
        }

        out.sort_by_key(|(attempts, when_received, _)| (*attempts, *when_received));
        out.truncate(limit as usize);
        Ok(out.into_iter().map(|(_, _, op)| op).collect())
    }
}

impl DhtStore<DbWrite<Dht>> {
    /// Returns `true` if `hash` appears in any op-bearing DHT table.
    ///
    /// Delegates to the read-only view of this store.
    pub async fn op_exists(&self, hash: &DhtOpHash) -> StateQueryResult<bool> {
        self.as_read().op_exists(hash).await
    }

    /// Drop any op whose hash is already recorded in the DHT store.
    ///
    /// Delegates to the read-only view of this store.
    pub async fn filter_existing_ops(
        &self,
        ops: Vec<DhtOpHashed>,
    ) -> StateQueryResult<Vec<DhtOpHashed>> {
        self.as_read().filter_existing_ops(ops).await
    }

    /// See [`DhtStore::ops_pending_sys_validation`].
    pub async fn ops_pending_sys_validation(
        &self,
        limit: u32,
    ) -> StateQueryResult<Vec<DhtOpHashed>> {
        self.as_read().ops_pending_sys_validation(limit).await
    }

    /// See [`DhtStore::find_fork_for_action`].
    pub async fn find_fork_for_action(
        &self,
        action: &holochain_zome_types::action::Action,
    ) -> StateQueryResult<Option<holochain_types::prelude::SignedAction>> {
        self.as_read().find_fork_for_action(action).await
    }
}

// Compile-only sanity check that the read-only alias resolves correctly.
#[allow(dead_code)]
fn _read_only_alias_compiles(_: DhtStoreRead) {}

/// Reconstruct a [`DhtOpHashed`] (`ChainOp` variant) from a `LimboChainOp`
/// row by fetching the action (and entry when required) from the database.
async fn chain_op_from_limbo_row(
    db: &DbRead<Dht>,
    row: &holochain_data::models::dht::LimboChainOpRow,
) -> StateQueryResult<DhtOpHashed> {
    use holochain_types::action::NewEntryAction;
    use holochain_types::dht_op::{ChainOp, DhtOp};
    use holochain_types::prelude::{RecordEntry, Signature};
    use holochain_zome_types::action::Action;
    use holochain_zome_types::dht_v2::to_legacy_signed_action;
    use holochain_zome_types::op::ChainOpType;

    let op_type = ChainOpType::try_from(row.op_type).map_err(|n| {
        crate::query::StateQueryError::Other(format!("invalid op_type {n} in LimboChainOp row"))
    })?;

    let action_hash = holo_hash::ActionHash::from_raw_36(row.action_hash.clone());
    let v2_action = db.get_action(action_hash.clone()).await?.ok_or_else(|| {
        crate::query::StateQueryError::Other(format!(
            "Action {action_hash:?} referenced by LimboChainOp not found"
        ))
    })?;

    let legacy = to_legacy_signed_action(&v2_action).map_err(|e| {
        crate::query::StateQueryError::Other(format!("to_legacy_signed_action: {e}"))
    })?;
    let signature: Signature = legacy.signature().clone();
    let action: Action = legacy.action().clone();

    // Look up the entry referenced by the given action, returning
    // `RecordEntry::Present` when found or `RecordEntry::NA` when the action
    // carries no entry hash.
    async fn fetch_entry_for_action(
        db: &DbRead<Dht>,
        action: &Action,
    ) -> StateQueryResult<RecordEntry> {
        match action.entry_hash() {
            None => Ok(RecordEntry::NA),
            Some(h) => {
                let entry = db.get_entry(h.clone(), None).await?.ok_or_else(|| {
                    crate::query::StateQueryError::Other(format!(
                        "Entry {h:?} referenced by Action not found"
                    ))
                })?;
                Ok(RecordEntry::Present(entry))
            }
        }
    }

    // Look up the entry referenced by an Update action, always returning
    // `RecordEntry::Present`.
    async fn fetch_entry_for_update(
        db: &DbRead<Dht>,
        update: &holochain_zome_types::action::Update,
    ) -> StateQueryResult<RecordEntry> {
        let entry_hash = update.entry_hash.clone();
        let entry = db
            .get_entry(entry_hash.clone(), None)
            .await?
            .ok_or_else(|| {
                crate::query::StateQueryError::Other(format!(
                    "Entry {entry_hash:?} for Update not found"
                ))
            })?;
        Ok(RecordEntry::Present(entry))
    }

    let chain_op = match op_type {
        ChainOpType::StoreRecord => {
            let entry = fetch_entry_for_action(db, &action).await?;
            ChainOp::StoreRecord(signature, action, entry)
        }
        ChainOpType::StoreEntry => {
            let entry_hash = action.entry_hash().cloned().ok_or_else(|| {
                crate::query::StateQueryError::Other("StoreEntry action has no entry_hash".into())
            })?;
            let entry = db
                .get_entry(entry_hash.clone(), None)
                .await?
                .ok_or_else(|| {
                    crate::query::StateQueryError::Other(format!(
                        "Entry {entry_hash:?} for StoreEntry not found"
                    ))
                })?;
            let new_entry_action = NewEntryAction::try_from(action).map_err(|_| {
                crate::query::StateQueryError::Other(
                    "StoreEntry action is not a Create/Update".into(),
                )
            })?;
            ChainOp::StoreEntry(signature, new_entry_action, entry)
        }
        ChainOpType::RegisterAgentActivity => ChainOp::RegisterAgentActivity(signature, action),
        ChainOpType::RegisterUpdatedContent => {
            let update = match action {
                Action::Update(u) => u,
                _ => {
                    return Err(crate::query::StateQueryError::Other(
                        "RegisterUpdatedContent action is not Update".into(),
                    ))
                }
            };
            let entry = fetch_entry_for_update(db, &update).await?;
            ChainOp::RegisterUpdatedContent(signature, update, entry)
        }
        ChainOpType::RegisterUpdatedRecord => {
            let update = match action {
                Action::Update(u) => u,
                _ => {
                    return Err(crate::query::StateQueryError::Other(
                        "RegisterUpdatedRecord action is not Update".into(),
                    ))
                }
            };
            let entry = fetch_entry_for_update(db, &update).await?;
            ChainOp::RegisterUpdatedRecord(signature, update, entry)
        }
        ChainOpType::RegisterDeletedEntryAction => {
            let delete = match action {
                Action::Delete(d) => d,
                _ => {
                    return Err(crate::query::StateQueryError::Other(
                        "RegisterDeletedEntryAction action is not Delete".into(),
                    ))
                }
            };
            ChainOp::RegisterDeletedEntryAction(signature, delete)
        }
        ChainOpType::RegisterDeletedBy => {
            let delete = match action {
                Action::Delete(d) => d,
                _ => {
                    return Err(crate::query::StateQueryError::Other(
                        "RegisterDeletedBy action is not Delete".into(),
                    ))
                }
            };
            ChainOp::RegisterDeletedBy(signature, delete)
        }
        ChainOpType::RegisterAddLink => {
            let create_link = match action {
                Action::CreateLink(c) => c,
                _ => {
                    return Err(crate::query::StateQueryError::Other(
                        "RegisterAddLink action is not CreateLink".into(),
                    ))
                }
            };
            ChainOp::RegisterAddLink(signature, create_link)
        }
        ChainOpType::RegisterRemoveLink => {
            let delete_link = match action {
                Action::DeleteLink(d) => d,
                _ => {
                    return Err(crate::query::StateQueryError::Other(
                        "RegisterRemoveLink action is not DeleteLink".into(),
                    ))
                }
            };
            ChainOp::RegisterRemoveLink(signature, delete_link)
        }
    };

    let op = DhtOp::ChainOp(Box::new(chain_op));
    let op_hash = holo_hash::DhtOpHash::from_raw_36(row.hash.clone());
    Ok(DhtOpHashed::with_pre_hashed(op, op_hash))
}

/// Reconstruct a [`DhtOpHashed`] (`WarrantOp` variant) from a `LimboWarrant` row.
fn warrant_from_limbo_row(
    row: &holochain_data::models::dht::LimboWarrantRow,
) -> StateQueryResult<DhtOpHashed> {
    use holochain_types::dht_op::DhtOp;
    use holochain_types::prelude::Signature;
    use holochain_types::warrant::WarrantOp;
    use holochain_zome_types::warrant::{SignedWarrant, Warrant, WarrantProof};

    let proof: WarrantProof = holochain_serialized_bytes::decode(&row.proof)?;
    let author = holo_hash::AgentPubKey::from_raw_36(row.author.clone());
    let warrantee = holo_hash::AgentPubKey::from_raw_36(row.warrantee.clone());
    let timestamp = holochain_types::prelude::Timestamp::from_micros(row.timestamp);

    let warrant = Warrant::new(proof, author, timestamp, warrantee);
    // The signature is not stored in the limbo row — warrants are self-proving
    // via their proof content.  Use a zeroed signature as a placeholder, the
    // same approach used elsewhere in the codebase when reconstructing
    // WarrantOps from storage without a separate signature column.
    let signature = Signature::from([0u8; 64]);
    let signed_warrant = SignedWarrant::new(warrant, signature);
    let warrant_op = WarrantOp::from(signed_warrant);
    let op = DhtOp::WarrantOp(Box::new(warrant_op));
    let op_hash = holo_hash::DhtOpHash::from_raw_36(row.hash.clone());
    Ok(DhtOpHashed::with_pre_hashed(op, op_hash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use holo_hash::{ActionHash, AgentPubKey, DhtOpHash, EntryHash, HoloHashed};
    use holochain_data::kind::Dht;
    use holochain_types::dht_op::{ChainOp, DhtOp, DhtOpHashed};
    use holochain_types::prelude::Signature;
    use holochain_types::prelude::Timestamp;
    use holochain_zome_types::action::{Action, Create, EntryType};
    use holochain_zome_types::entry_def::EntryVisibility;
    use holochain_zome_types::prelude::AppEntryDef;
    use std::sync::Arc;

    fn make_fork_op(author: &AgentPubKey, prev: &ActionHash, seq: u32, seed: u8) -> DhtOpHashed {
        let action = Action::Create(Create {
            author: author.clone(),
            timestamp: Timestamp::from_micros(seed as i64 * 1000),
            action_seq: seq,
            prev_action: prev.clone(),
            entry_type: EntryType::App(AppEntryDef::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            )),
            entry_hash: EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]),
            weight: Default::default(),
        });
        let chain_op = ChainOp::RegisterAgentActivity(Signature::from([seed; 64]), action);
        DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(chain_op)))
    }

    fn dht_id() -> Dht {
        Dht::new(Arc::new(holo_hash::DnaHash::from_raw_36(vec![0u8; 36])))
    }

    fn make_chain_op(seed: u8) -> DhtOpHashed {
        let author = AgentPubKey::from_raw_36(vec![seed; 36]);
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
        let chain_op = ChainOp::RegisterAgentActivity(Signature::from([seed; 64]), action);
        DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(chain_op)))
    }

    /// Build a synthetic `DhtOpHashed` with the given pre-computed hash.
    fn make_chain_op_with_hash(seed: u8, hash: DhtOpHash) -> DhtOpHashed {
        let op = make_chain_op(seed);
        HoloHashed::with_pre_hashed(op.into_inner().0, hash)
    }

    #[tokio::test]
    async fn op_exists_returns_false_for_unknown_hash() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let unknown = DhtOpHash::from_raw_36(vec![99u8; 36]);
        let exists = store.op_exists(&unknown).await.unwrap();
        assert!(!exists);
    }

    #[tokio::test]
    async fn op_exists_returns_true_after_record_incoming_ops() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let op = make_chain_op(1);
        let hash = op.as_hash().clone();

        store.record_incoming_ops(vec![op]).await.unwrap();

        let exists = store.op_exists(&hash).await.unwrap();
        assert!(exists, "op_exists should be true after record_incoming_ops");
    }

    #[tokio::test]
    async fn filter_existing_ops_removes_known_hashes() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let known = make_chain_op(2);
        let unknown = make_chain_op(3);
        let known_hash = known.as_hash().clone();
        let unknown_hash = unknown.as_hash().clone();

        store.record_incoming_ops(vec![known]).await.unwrap();

        let input = vec![make_chain_op_with_hash(20, known_hash.clone()), unknown];
        let filtered = store.as_read().filter_existing_ops(input).await.unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].as_hash(), &unknown_hash);
    }

    #[tokio::test]
    async fn ops_pending_sys_validation_returns_recorded_chain_op() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let op = make_chain_op(10);
        let hash = op.as_hash().clone();

        store.record_incoming_ops(vec![op]).await.unwrap();

        let pending = store.ops_pending_sys_validation(1_000).await.unwrap();
        let hashes: Vec<_> = pending.iter().map(|o| o.as_hash().clone()).collect();
        assert!(hashes.contains(&hash));
    }

    #[tokio::test]
    async fn ops_pending_sys_validation_excludes_completed() {
        use crate::dht_store::SysOutcome;

        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let op = make_chain_op(11);
        let hash = op.as_hash().clone();

        store.record_incoming_ops(vec![op]).await.unwrap();
        store
            .record_chain_op_sys_validation_outcomes(vec![(hash.clone(), SysOutcome::Accepted)])
            .await
            .unwrap();

        let pending = store.ops_pending_sys_validation(1_000).await.unwrap();
        let hashes: Vec<_> = pending.iter().map(|o| o.as_hash().clone()).collect();
        assert!(!hashes.contains(&hash));
    }

    #[tokio::test]
    async fn ops_pending_sys_validation_respects_limit_across_union() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let ops: Vec<_> = (12..16).map(make_chain_op).collect();
        store.record_incoming_ops(ops).await.unwrap();

        let pending = store.ops_pending_sys_validation(2).await.unwrap();
        assert_eq!(pending.len(), 2);
    }

    #[tokio::test]
    async fn find_fork_for_action_returns_none_when_no_sibling() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let op = make_chain_op(30);
        let action = match op.as_content() {
            DhtOp::ChainOp(c) => c.action().clone(),
            _ => unreachable!(),
        };

        let result = store.find_fork_for_action(&action).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn find_fork_for_action_returns_sibling() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();

        let author = AgentPubKey::from_raw_36(vec![31u8; 36]);
        let prev_action_hash = ActionHash::from_raw_36(vec![231u8; 36]);

        // Two ops sharing the same prev_action and author but with different
        // timestamps/entry_hashes (seeds differ) — different hashes overall.
        let op_a = make_fork_op(&author, &prev_action_hash, 2, 32);
        let op_b = make_fork_op(&author, &prev_action_hash, 2, 33);

        let action_b = match op_b.as_content() {
            DhtOp::ChainOp(c) => c.action().clone(),
            _ => unreachable!(),
        };

        store.record_incoming_ops(vec![op_a]).await.unwrap();

        let result = store.find_fork_for_action(&action_b).await.unwrap();
        assert!(result.is_some(), "fork should be detected");
    }
}
