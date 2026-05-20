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
use holochain_zome_types::dht_v2::RecordValidity;

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
    ///
    /// The returned tuple is `(hash, signature)` for the sibling action —
    /// the exact shape the sys-validation workflow needs to author a
    /// `ChainFork` warrant.
    pub async fn find_fork_for_action(
        &self,
        action: &holochain_zome_types::action::Action,
    ) -> StateQueryResult<Option<(holo_hash::ActionHash, holochain_types::prelude::Signature)>>
    {
        let Some(prev) = action.prev_action() else {
            return Ok(None);
        };
        let incoming_hash = holo_hash::ActionHash::with_data_sync(action);

        let siblings = self
            .db()
            .get_actions_by_prev_hash(prev, &incoming_hash)
            .await?;

        let incoming_author = action.author();
        if let Some(sibling) = siblings.into_iter().next() {
            let existing_author = &sibling.hashed.content.header.author;
            if existing_author != incoming_author {
                return Err(crate::query::StateQueryError::Other(format!(
                    "Cross-author prev_action collision: incoming author {incoming_author} \
                     differs from existing author {existing_author} for prev_action {prev:?}"
                )));
            }
            let hash = sibling.as_hash().clone();
            let signature = sibling.signature().clone();
            return Ok(Some((hash, signature)));
        }
        Ok(None)
    }

    /// Return chain ops that have passed system validation and are awaiting
    /// app validation. Warrants have no app-validation stage, so they are not
    /// included.
    pub async fn ops_pending_app_validation(
        &self,
        limit: u32,
    ) -> StateQueryResult<Vec<DhtOpHashed>> {
        let db = self.db();
        let rows = db
            .limbo_chain_ops_pending_app_validation_with_action(limit)
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(chain_op_from_joined_row(&row)?);
        }
        Ok(out)
    }

    /// Return pending validation receipts: one entry per integrated, validated
    /// op that still has `require_receipt = 1`. Each tuple is
    /// `(ValidationReceipt, action_author)` so the caller can group by author.
    ///
    /// The `validators` field on each [`ValidationReceipt`] is populated from
    /// the supplied `validators` argument (the locally-running agents for this
    /// DNA).
    pub async fn pending_validation_receipts(
        &self,
        validators: Vec<holo_hash::AgentPubKey>,
    ) -> StateQueryResult<
        Vec<(
            holochain_types::prelude::ValidationReceipt,
            holo_hash::AgentPubKey,
        )>,
    > {
        let rows = self.db().pending_validation_receipts().await?;
        rows.into_iter()
            .map(|r| {
                let dht_op_hash = holo_hash::DhtOpHash::from_raw_36(r.op_hash);
                let author = holo_hash::AgentPubKey::from_raw_36(r.action_author);
                // Map from RecordValidity (Accepted=1, Rejected=2) to
                // ValidationStatus (Valid=0, Rejected=1).
                let record_validity =
                    RecordValidity::try_from(r.validation_status).map_err(|v| {
                        crate::query::StateQueryError::Other(format!(
                            "invalid validation_status {v} in ChainOp row"
                        ))
                    })?;
                let validation_status = match record_validity {
                    RecordValidity::Accepted => {
                        holochain_zome_types::validate::ValidationStatus::Valid
                    }
                    RecordValidity::Rejected => {
                        holochain_zome_types::validate::ValidationStatus::Rejected
                    }
                };
                let when_integrated =
                    holochain_types::prelude::Timestamp::from_micros(r.when_integrated);
                Ok((
                    holochain_types::prelude::ValidationReceipt {
                        dht_op_hash,
                        validation_status,
                        validators: validators.clone(),
                        when_integrated,
                    },
                    author,
                ))
            })
            .collect()
    }

    /// Return ops awaiting system validation, sorted across chain ops and
    /// warrants by `(sys_validation_attempts, when_received)`, up to `limit`
    /// rows total.
    pub async fn ops_pending_sys_validation(
        &self,
        limit: u32,
    ) -> StateQueryResult<Vec<DhtOpHashed>> {
        let db = self.db();
        let chain_rows = db
            .limbo_chain_ops_pending_sys_validation_with_action(limit)
            .await?;
        let warrant_rows = db.limbo_warrants_pending_sys_validation(limit).await?;

        let mut out: Vec<(i64, i64, DhtOpHashed)> =
            Vec::with_capacity(chain_rows.len() + warrant_rows.len());

        for row in chain_rows {
            let attempts = row.sys_validation_attempts;
            let when_received = row.when_received;
            let op = chain_op_from_joined_row(&row)?;
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

    /// See [`DhtStore::pending_validation_receipts`].
    pub async fn pending_validation_receipts(
        &self,
        validators: Vec<holo_hash::AgentPubKey>,
    ) -> StateQueryResult<
        Vec<(
            holochain_types::prelude::ValidationReceipt,
            holo_hash::AgentPubKey,
        )>,
    > {
        self.as_read().pending_validation_receipts(validators).await
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

    /// See [`DhtStore::ops_pending_app_validation`].
    pub async fn ops_pending_app_validation(
        &self,
        limit: u32,
    ) -> StateQueryResult<Vec<DhtOpHashed>> {
        self.as_read().ops_pending_app_validation(limit).await
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
    ) -> StateQueryResult<Option<(holo_hash::ActionHash, holochain_types::prelude::Signature)>>
    {
        self.as_read().find_fork_for_action(action).await
    }
}

// Compile-only sanity check that the read-only alias resolves correctly.
#[allow(dead_code)]
fn _read_only_alias_compiles(_: DhtStoreRead) {}

/// Reconstruct a [`DhtOpHashed`] (`ChainOp` variant) from a
/// [`LimboChainOpJoinedRow`] without any additional database round-trips.
/// The action and entry blobs are decoded in-process from the joined columns.
fn chain_op_from_joined_row(
    row: &holochain_data::dht::LimboChainOpJoinedRow,
) -> StateQueryResult<DhtOpHashed> {
    use holo_hash::{ActionHash, AgentPubKey};
    use holochain_types::action::NewEntryAction;
    use holochain_types::dht_op::{ChainOp, DhtOp};
    use holochain_types::prelude::{RecordEntry, Signature};
    use holochain_zome_types::action::Action as LegacyAction;
    use holochain_zome_types::dht_v2::{
        to_legacy_signed_action, Action, ActionData, ActionHeader, SignedActionHashed,
    };
    use holochain_zome_types::op::ChainOpType;
    use holochain_zome_types::prelude::Signature as V2Signature;

    let op_type = ChainOpType::try_from(row.op_type).map_err(|n| {
        crate::query::StateQueryError::Other(format!("invalid op_type {n} in LimboChainOp row"))
    })?;

    // Decode the v2 Action from the joined columns.
    let action_data: ActionData = holochain_serialized_bytes::decode(&row.action_data)
        .map_err(|e| crate::query::StateQueryError::Other(format!("decode ActionData: {e}")))?;
    let action_v2 = Action {
        header: ActionHeader {
            author: AgentPubKey::from_raw_36(row.action_author.clone()),
            timestamp: holochain_types::prelude::Timestamp::from_micros(row.action_timestamp),
            action_seq: row.action_seq as u32,
            prev_action: row
                .action_prev_hash
                .as_ref()
                .map(|h| ActionHash::from_raw_36(h.clone())),
        },
        data: action_data,
    };
    let sig_bytes: [u8; 64] = row.action_signature.as_slice().try_into().map_err(|_| {
        crate::query::StateQueryError::Other(format!(
            "signature column has {} bytes, expected 64",
            row.action_signature.len()
        ))
    })?;
    let action_hash = ActionHash::from_raw_36(row.action_hash.clone());
    let hashed = holo_hash::HoloHashed::with_pre_hashed(action_v2, action_hash);
    let v2_signed: SignedActionHashed =
        SignedActionHashed::with_presigned(hashed, V2Signature(sig_bytes));

    let legacy = to_legacy_signed_action(&v2_signed).map_err(|e| {
        crate::query::StateQueryError::Other(format!("to_legacy_signed_action: {e}"))
    })?;
    let signature: Signature = legacy.signature().clone();
    let action: LegacyAction = legacy.action().clone();

    // Decode the optional entry blob from the LEFT JOIN.
    let decoded_entry: Option<holochain_types::prelude::Entry> = row
        .entry_blob
        .as_ref()
        .map(|blob| {
            holochain_serialized_bytes::decode(blob)
                .map_err(|e| crate::query::StateQueryError::Other(format!("decode Entry: {e}")))
        })
        .transpose()?;

    // Helper: build RecordEntry from legacy action + decoded entry.
    let entry_for_action = |action: &LegacyAction| -> StateQueryResult<RecordEntry> {
        use holochain_zome_types::entry_def::EntryVisibility;
        if action.entry_hash().is_none() {
            return Ok(RecordEntry::NA);
        }
        match decoded_entry.clone() {
            Some(entry) => Ok(RecordEntry::Present(entry)),
            None => {
                if action.entry_visibility() == Some(&EntryVisibility::Private) {
                    Ok(RecordEntry::Hidden)
                } else {
                    Ok(RecordEntry::NotStored)
                }
            }
        }
    };

    // Helper: entry required to be present (Update ops).
    let entry_for_update =
        |update: &holochain_zome_types::action::Update| -> StateQueryResult<RecordEntry> {
            match decoded_entry.clone() {
                Some(entry) => Ok(RecordEntry::Present(entry)),
                None => Err(crate::query::StateQueryError::Other(format!(
                    "Entry {:?} for Update not found",
                    update.entry_hash
                ))),
            }
        };

    let chain_op = match op_type {
        ChainOpType::StoreRecord => {
            let entry = entry_for_action(&action)?;
            ChainOp::StoreRecord(signature, action, entry)
        }
        ChainOpType::StoreEntry => {
            let entry_hash = action.entry_hash().cloned().ok_or_else(|| {
                crate::query::StateQueryError::Other("StoreEntry action has no entry_hash".into())
            })?;
            let entry = decoded_entry.clone().ok_or_else(|| {
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
                LegacyAction::Update(u) => u,
                _ => {
                    return Err(crate::query::StateQueryError::Other(
                        "RegisterUpdatedContent action is not Update".into(),
                    ))
                }
            };
            let entry = entry_for_update(&update)?;
            ChainOp::RegisterUpdatedContent(signature, update, entry)
        }
        ChainOpType::RegisterUpdatedRecord => {
            let update = match action {
                LegacyAction::Update(u) => u,
                _ => {
                    return Err(crate::query::StateQueryError::Other(
                        "RegisterUpdatedRecord action is not Update".into(),
                    ))
                }
            };
            let entry = entry_for_update(&update)?;
            ChainOp::RegisterUpdatedRecord(signature, update, entry)
        }
        ChainOpType::RegisterDeletedEntryAction => {
            let delete = match action {
                LegacyAction::Delete(d) => d,
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
                LegacyAction::Delete(d) => d,
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
                LegacyAction::CreateLink(c) => c,
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
                LegacyAction::DeleteLink(d) => d,
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
    async fn ops_pending_app_validation_returns_sys_validated_chain_op() {
        use crate::dht_store::SysOutcome;

        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let op = make_chain_op(50);
        let hash = op.as_hash().clone();

        store.record_incoming_ops(vec![op]).await.unwrap();
        store
            .record_chain_op_sys_validation_outcomes(vec![(hash.clone(), SysOutcome::Accepted)])
            .await
            .unwrap();

        let pending = store.ops_pending_app_validation(1_000).await.unwrap();
        let hashes: Vec<_> = pending.iter().map(|o| o.as_hash().clone()).collect();
        assert!(hashes.contains(&hash));
    }

    #[tokio::test]
    async fn ops_pending_app_validation_excludes_pending_sys() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let op = make_chain_op(51);
        let hash = op.as_hash().clone();

        // Insert but don't record sys-validation outcome.
        store.record_incoming_ops(vec![op]).await.unwrap();

        let pending = store.ops_pending_app_validation(1_000).await.unwrap();
        let hashes: Vec<_> = pending.iter().map(|o| o.as_hash().clone()).collect();
        assert!(
            !hashes.contains(&hash),
            "op not yet sys-validated should not appear"
        );
    }

    #[tokio::test]
    async fn ops_pending_app_validation_excludes_app_validated() {
        use crate::dht_store::{AppOutcome, SysOutcome};

        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let op = make_chain_op(52);
        let hash = op.as_hash().clone();

        store.record_incoming_ops(vec![op]).await.unwrap();
        store
            .record_chain_op_sys_validation_outcomes(vec![(hash.clone(), SysOutcome::Accepted)])
            .await
            .unwrap();
        store
            .record_app_validation_outcomes(vec![(hash.clone(), AppOutcome::Accepted)])
            .await
            .unwrap();

        let pending = store.ops_pending_app_validation(1_000).await.unwrap();
        let hashes: Vec<_> = pending.iter().map(|o| o.as_hash().clone()).collect();
        assert!(
            !hashes.contains(&hash),
            "fully-validated op should not appear"
        );
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

        // Capture op_a's action hash and signature before moving op_a into the store.
        let expected_hash = match op_a.as_content() {
            DhtOp::ChainOp(c) => ActionHash::with_data_sync(&c.action()),
            _ => unreachable!(),
        };
        let expected_sig = match op_a.as_content() {
            DhtOp::ChainOp(c) => c.signature().clone(),
            _ => unreachable!(),
        };

        let action_b = match op_b.as_content() {
            DhtOp::ChainOp(c) => c.action().clone(),
            _ => unreachable!(),
        };

        store.record_incoming_ops(vec![op_a]).await.unwrap();

        let result = store.find_fork_for_action(&action_b).await.unwrap();
        let (got_hash, got_sig) = result.expect("fork should be detected");
        assert_eq!(got_hash, expected_hash, "sibling hash should match op_a");
        assert_eq!(got_sig, expected_sig, "sibling signature should match op_a");
    }

    #[tokio::test]
    async fn pending_validation_receipts_returns_integrated_require_receipt_ops() {
        use crate::dht_store::{AppOutcome, SysOutcome};

        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let op = make_chain_op(60);
        let hash = op.as_hash().clone();
        let author = match op.as_content() {
            DhtOp::ChainOp(c) => c.action().author().clone(),
            _ => unreachable!(),
        };

        store.record_incoming_ops(vec![op]).await.unwrap();
        store
            .record_chain_op_sys_validation_outcomes(vec![(hash.clone(), SysOutcome::Accepted)])
            .await
            .unwrap();
        store
            .record_app_validation_outcomes(vec![(hash.clone(), AppOutcome::Accepted)])
            .await
            .unwrap();
        store
            .integrate_ready_ops(holochain_types::prelude::Timestamp::now())
            .await
            .unwrap();

        let validators = vec![AgentPubKey::from_raw_36(vec![0xFF; 36])];
        let receipts = store
            .pending_validation_receipts(validators.clone())
            .await
            .unwrap();

        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].0.dht_op_hash, hash);
        assert_eq!(receipts[0].1, author);
        assert_eq!(receipts[0].0.validators, validators);
    }

    #[tokio::test]
    async fn pending_validation_receipts_excludes_ops_without_require_receipt() {
        use crate::dht_store::{AppOutcome, SysOutcome};

        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let op = make_chain_op(61);
        let hash = op.as_hash().clone();
        store.record_incoming_ops(vec![op]).await.unwrap();
        store
            .record_chain_op_sys_validation_outcomes(vec![(hash.clone(), SysOutcome::Accepted)])
            .await
            .unwrap();
        store
            .record_app_validation_outcomes(vec![(hash.clone(), AppOutcome::Accepted)])
            .await
            .unwrap();
        store
            .integrate_ready_ops(holochain_types::prelude::Timestamp::now())
            .await
            .unwrap();
        store
            .clear_require_receipts(vec![hash.clone()])
            .await
            .unwrap();

        let receipts = store.pending_validation_receipts(vec![]).await.unwrap();
        assert!(
            receipts.iter().all(|(r, _)| r.dht_op_hash != hash),
            "op with cleared require_receipt should not appear"
        );
    }
}
