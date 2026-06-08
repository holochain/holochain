//! Read operations on the per-DNA DHT store.
//!
//! Methods on [`DhtStoreRead`] expose domain-meaningful reads that delegate to
//! `holochain_data`'s `DbRead<Dht>` primitives and return values in terms of
//! the project's existing domain types. The parent module holds the
//! corresponding write operations.

use super::DhtStore;
use crate::prelude::ActionSequenceAndHash;
use crate::query::StateQueryResult;
use holo_hash::{DhtOpHash, HasHash};
use holochain_data::kind::Dht;
use holochain_data::DbRead;
use holochain_types::chain::ChainItem;
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::prelude::{
    ActionHashedContainer, AgentActivityResponse, ChainItems, ChainItemsSource,
    MustGetAgentActivityResponse, RegisterAgentActivity,
};
use holochain_types::warrant::WarrantOp;
use holochain_zome_types::chain::{ChainFilter, LimitConditions};
use holochain_zome_types::dht_v2::RecordValidity;
use holochain_zome_types::prelude::{
    ChainFork, ChainHead, ChainQueryFilter, ChainStatus, HighestObserved, SignedWarrant,
};
use holochain_zome_types::validate::ValidationStatus;
use std::collections::{HashMap, HashSet};

impl DhtStore<DbRead<Dht>> {
    /// Returns `true` if `hash` appears in any op-bearing DHT table
    /// (`ChainOp`, `LimboChainOp`, `WarrantOp`, `LimboWarrantOp`).
    pub async fn op_exists(&self, hash: &DhtOpHash) -> StateQueryResult<bool> {
        Ok(self.db().op_exists(hash).await?)
    }

    /// Count integrated ops (chain ops plus warrants) held in the DHT store.
    pub async fn count_integrated_ops(&self) -> StateQueryResult<i64> {
        Ok(self.db().count_integrated_ops().await?)
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

    /// Retrieve the entry for `hash` if present. `author = Some` includes that
    /// agent's private entry; `None` returns public entries only.
    pub async fn retrieve_entry(
        &self,
        hash: &holo_hash::EntryHash,
        author: Option<&holo_hash::AgentPubKey>,
    ) -> StateQueryResult<Option<holochain_types::prelude::Entry>> {
        Ok(self.db().get_entry(hash.clone(), author).await?)
    }

    /// Retrieve the complete record (action + entry, if any) for `hash`.
    ///
    /// If the action references an entry and that entry is not available
    /// locally, returns `None` (matching the cascade's `retrieve_public_record`
    /// contract). `author = Some` allows that agent's private entry.
    pub async fn retrieve_record(
        &self,
        hash: &holo_hash::ActionHash,
        author: Option<&holo_hash::AgentPubKey>,
    ) -> StateQueryResult<Option<holochain_zome_types::record::Record>> {
        let Some(v2_action) = self.db().get_action(hash.clone()).await? else {
            return Ok(None);
        };
        let action = holochain_zome_types::dht_v2::to_legacy_signed_action(&v2_action);
        let entry = match action.action().entry_hash() {
            Some(entry_hash) => match self.db().get_entry(entry_hash.clone(), author).await? {
                Some(entry) => Some(entry),
                // Action references an entry but it is unavailable.
                None => return Ok(None),
            },
            None => None,
        };
        Ok(Some(holochain_zome_types::record::Record::new(
            action, entry,
        )))
    }

    /// Retrieve the record for `hash` only if it is still live — i.e. its
    /// action exists and no integrated `Delete` targets it. Returns `None` if
    /// the action is absent, deleted, or (per `retrieve_record`) references an
    /// entry that is unavailable. `author = Some` allows that agent's private
    /// entry.
    pub async fn get_live_record(
        &self,
        hash: &holo_hash::ActionHash,
        author: Option<&holo_hash::AgentPubKey>,
    ) -> StateQueryResult<Option<holochain_zome_types::record::Record>> {
        if !self
            .db()
            .get_deleted_records(hash.clone())
            .await?
            .is_empty()
        {
            return Ok(None);
        }
        self.retrieve_record(hash, author).await
    }

    /// The live `Record` for an entry: among the entry's valid, integrated,
    /// undeleted `StoreEntry` creates (visible to `author`), prefer the one
    /// authored by `author`, else the first by integration order. Returns
    /// `None` if there are no live creates. `author = Some` includes that
    /// agent's private entry.
    pub async fn get_live_entry(
        &self,
        entry_hash: &holo_hash::EntryHash,
        author: Option<&holo_hash::AgentPubKey>,
    ) -> StateQueryResult<Option<holochain_zome_types::record::Record>> {
        let creates = self.db().get_live_entry_creates(entry_hash, author).await?;
        let chosen = match author {
            Some(a) => {
                let authored = creates
                    .iter()
                    .find(|sah| &sah.hashed.content.header.author == a)
                    .cloned();
                authored.or_else(|| creates.into_iter().next())
            }
            None => creates.into_iter().next(),
        };
        let Some(v2_sah) = chosen else {
            return Ok(None);
        };
        let action = holochain_zome_types::dht_v2::to_legacy_signed_action(&v2_sah);
        let entry = self.db().get_entry(entry_hash.clone(), author).await?;
        Ok(Some(holochain_zome_types::record::Record::new(
            action, entry,
        )))
    }

    /// Assemble the [`EntryDetails`] for `entry_hash`: the entry, its valid
    /// create actions, rejected create actions, the deletes on it, the updates
    /// from it, and its Live/Dead status. Returns `None` if the entry is not
    /// available. `author = Some` allows that agent's private entry.
    pub async fn get_entry_details(
        &self,
        entry_hash: &holo_hash::EntryHash,
        author: Option<&holo_hash::AgentPubKey>,
    ) -> StateQueryResult<Option<holochain_zome_types::metadata::EntryDetails>> {
        let Some(entry) = self.db().get_entry(entry_hash.clone(), author).await? else {
            return Ok(None);
        };
        let to_legacy = holochain_zome_types::dht_v2::to_legacy_signed_action;
        let actions = self
            .db()
            .get_entry_creates(entry_hash, author, i64::from(RecordValidity::Accepted))
            .await?
            .iter()
            .map(to_legacy)
            .collect();
        let rejected_actions = self
            .db()
            .get_entry_creates(entry_hash, author, i64::from(RecordValidity::Rejected))
            .await?
            .iter()
            .map(to_legacy)
            .collect();
        let deletes = self
            .db()
            .get_delete_actions_for_entry(entry_hash)
            .await?
            .iter()
            .map(to_legacy)
            .collect();
        let updates = self
            .db()
            .get_update_actions_for_entry(entry_hash)
            .await?
            .iter()
            .map(to_legacy)
            .collect();
        let entry_dht_status = if self
            .db()
            .get_live_entry_creates(entry_hash, author)
            .await?
            .is_empty()
        {
            holochain_zome_types::metadata::EntryDhtStatus::Dead
        } else {
            holochain_zome_types::metadata::EntryDhtStatus::Live
        };
        Ok(Some(holochain_zome_types::metadata::EntryDetails {
            entry,
            actions,
            rejected_actions,
            deletes,
            updates,
            entry_dht_status,
        }))
    }

    /// Retrieve the signed action for `hash` if present, without CRUD
    /// resolution. Returns the legacy `SignedActionHashed` (converted from the
    /// stored v2 action).
    pub async fn retrieve_action(
        &self,
        hash: &holo_hash::ActionHash,
    ) -> StateQueryResult<Option<holochain_zome_types::record::SignedActionHashed>> {
        Ok(self
            .db()
            .get_action(hash.clone())
            .await?
            .map(|v2| holochain_zome_types::dht_v2::to_legacy_signed_action(&v2)))
    }

    /// Assemble the [`RecordDetails`] for `hash`: the record, its validation
    /// status (from its integrated `StoreRecord` op), the deletes targeting it,
    /// and the updates of it. Returns `None` if there is no integrated
    /// `StoreRecord` op for `hash`. `author = Some` allows that agent's private
    /// entry.
    pub async fn get_record_details(
        &self,
        hash: &holo_hash::ActionHash,
        author: Option<&holo_hash::AgentPubKey>,
    ) -> StateQueryResult<Option<holochain_zome_types::metadata::RecordDetails>> {
        use holochain_zome_types::op::ChainOpType;
        let ops = self.db().get_chain_ops_for_action(hash.clone()).await?;
        let Some(store_op) = ops
            .iter()
            .find(|r| ChainOpType::try_from(r.op_type) == Ok(ChainOpType::StoreRecord))
        else {
            return Ok(None);
        };
        let validation_status = match RecordValidity::try_from(store_op.validation_status) {
            Ok(RecordValidity::Accepted) => holochain_zome_types::validate::ValidationStatus::Valid,
            Ok(RecordValidity::Rejected) => {
                holochain_zome_types::validate::ValidationStatus::Rejected
            }
            Err(v) => {
                return Err(crate::query::StateQueryError::Other(format!(
                    "invalid validation_status {v} on StoreRecord op for {hash:?}"
                )))
            }
        };
        let Some(record) = self.retrieve_record(hash, author).await? else {
            return Ok(None);
        };
        let deletes = self
            .db()
            .get_delete_actions_for_record(hash)
            .await?
            .iter()
            .map(holochain_zome_types::dht_v2::to_legacy_signed_action)
            .collect();
        let updates = self
            .db()
            .get_update_actions_for_record(hash)
            .await?
            .iter()
            .map(holochain_zome_types::dht_v2::to_legacy_signed_action)
            .collect();
        Ok(Some(holochain_zome_types::metadata::RecordDetails {
            record,
            validation_status,
            deletes,
            updates,
        }))
    }

    /// For `base`, every `CreateLink` (live and tombstoned) matching
    /// `type_query`/`tag`, paired with its `DeleteLink`s. Returned as legacy
    /// `SignedActionHashed`s.
    pub async fn get_link_details(
        &self,
        base: &holo_hash::AnyLinkableHash,
        type_query: &holochain_zome_types::prelude::LinkTypeFilter,
        tag: Option<&holochain_zome_types::prelude::LinkTag>,
    ) -> StateQueryResult<
        Vec<(
            holochain_zome_types::record::SignedActionHashed,
            Vec<holochain_zome_types::record::SignedActionHashed>,
        )>,
    > {
        let creates = self.db().get_link_create_actions(base).await?;
        let mut out = Vec::with_capacity(creates.len());
        for create in creates {
            let holochain_zome_types::dht_v2::ActionData::CreateLink(d) =
                &create.hashed.content.data
            else {
                continue;
            };
            if !type_query.contains(&d.zome_index, &d.link_type) {
                continue;
            }
            if let Some(t) = tag {
                if !d.tag.0.starts_with(&t.0) {
                    continue;
                }
            }
            let deletes = self
                .db()
                .get_delete_link_actions(create.as_hash())
                .await?
                .iter()
                .map(holochain_zome_types::dht_v2::to_legacy_signed_action)
                .collect();
            out.push((
                holochain_zome_types::dht_v2::to_legacy_signed_action(&create),
                deletes,
            ));
        }
        Ok(out)
    }

    /// The live links on `base` (CreateLink minus DeleteLink tombstones),
    /// filtered by link type, tag prefix, author, and time. Builds each
    /// [`holochain_zome_types::link::Link`] from its `CreateLink` action.
    ///
    /// Time bounds mirror the legacy `LinksQuery`: `after` is inclusive
    /// (`timestamp >= after`) and `before` is inclusive (`timestamp <= before`).
    pub async fn get_links(
        &self,
        base: &holo_hash::AnyLinkableHash,
        type_query: &holochain_zome_types::prelude::LinkTypeFilter,
        tag: Option<&holochain_zome_types::prelude::LinkTag>,
        filter: &crate::query::link::GetLinksFilter,
    ) -> StateQueryResult<Vec<holochain_zome_types::link::Link>> {
        let actions = self.db().get_live_link_actions(base).await?;
        let mut links = Vec::with_capacity(actions.len());
        for sah in actions {
            let header = &sah.hashed.content.header;
            let holochain_zome_types::dht_v2::ActionData::CreateLink(d) = &sah.hashed.content.data
            else {
                continue;
            };
            if !type_query.contains(&d.zome_index, &d.link_type) {
                continue;
            }
            if let Some(t) = tag {
                if !d.tag.0.starts_with(&t.0) {
                    continue;
                }
            }
            if let Some(author) = &filter.author {
                if &header.author != author {
                    continue;
                }
            }
            if let Some(before) = filter.before {
                if header.timestamp > before {
                    continue;
                }
            }
            if let Some(after) = filter.after {
                if header.timestamp < after {
                    continue;
                }
            }
            links.push(holochain_zome_types::link::Link {
                author: header.author.clone(),
                base: d.base_address.clone(),
                target: d.target_address.clone(),
                timestamp: header.timestamp,
                zome_index: d.zome_index,
                link_type: d.link_type,
                tag: d.tag.clone(),
                create_link_hash: sah.as_hash().clone(),
            });
        }
        Ok(links)
    }

    /// Agent activity for `author`: the integrated `RegisterAgentActivity`
    /// actions classified into valid/rejected, with chain status, highest
    /// observed, and warrants. Store-only (no scratch). Returns legacy types.
    pub async fn get_agent_activity(
        &self,
        author: &holo_hash::AgentPubKey,
        filter: &ChainQueryFilter,
        options: &crate::dht_store::GetAgentActivityOptions,
    ) -> StateQueryResult<AgentActivityResponse> {
        use holochain_zome_types::dht_v2::to_legacy_signed_action;
        use holochain_zome_types::record::Record;

        let items = self
            .db()
            .get_agent_activity(author.clone(), options.include_full_records)
            .await?;

        let warrants = if options.include_warrants {
            self.db()
                .get_warrants_by_warrantee(author.clone())
                .await?
                .into_iter()
                .map(warrant_row_to_signed_warrant)
                .collect::<StateQueryResult<Vec<_>>>()?
        } else {
            Vec::new()
        };

        if options.include_full_records {
            let mut valid = Vec::new();
            let mut rejected = Vec::new();
            for item in items {
                let record = Record::new(to_legacy_signed_action(&item.action), item.entry);
                match item.validation_status {
                    RecordValidity::Accepted => valid.push(record),
                    RecordValidity::Rejected => rejected.push(record),
                }
            }
            Ok(build_agent_activity_response(
                author.clone(),
                valid,
                rejected,
                warrants,
                filter,
                options,
            ))
        } else {
            let mut valid = Vec::new();
            let mut rejected = Vec::new();
            for item in items {
                let action_hashed = to_legacy_signed_action(&item.action).hashed;
                match item.validation_status {
                    RecordValidity::Accepted => valid.push(action_hashed),
                    RecordValidity::Rejected => rejected.push(action_hashed),
                }
            }
            Ok(build_agent_activity_response(
                author.clone(),
                valid,
                rejected,
                warrants,
                filter,
                options,
            ))
        }
    }

    /// Store-only `must_get_agent_activity`: resolve `filter.chain_top`, scan the
    /// bounded `RegisterAgentActivity` range, exclude forked actions, apply the
    /// timestamp/take filters, and decide completeness. Returns legacy types.
    /// No scratch, no network, no cross-source merge (the requester layers those
    /// on in phase 1c).
    pub async fn must_get_agent_activity(
        &self,
        author: &holo_hash::AgentPubKey,
        filter: &ChainFilter,
    ) -> StateQueryResult<MustGetAgentActivityResponse> {
        use holochain_zome_types::dht_v2::to_legacy_signed_action;

        // A take of zero is a degenerate filter.
        if filter.get_take() == Some(0) {
            return Err(crate::query::StateQueryError::InvalidInput(
                "ChainFilter take must be greater than 0".to_string(),
            ));
        }

        // Resolve the chain top; if absent we cannot answer.
        let Some((chain_top_seq, chain_top_timestamp)) = self
            .db()
            .get_action_seq_and_timestamp(author.clone(), filter.chain_top.clone())
            .await?
        else {
            return Ok(MustGetAgentActivityResponse::ChainTopNotFound(
                filter.chain_top.clone(),
            ));
        };

        // An until_timestamp after the chain top is unsatisfiable.
        if let Some(until_timestamp) = filter.get_until_timestamp() {
            if until_timestamp > chain_top_timestamp {
                return Ok(
                    MustGetAgentActivityResponse::UntilTimestampGreaterThanChainHead(
                        until_timestamp,
                    ),
                );
            }
        }

        // Resolve the until_hash lower bound, if any.
        let mut resolved_until_seq = None;
        if let Some(until_hash) = filter.get_until_hash() {
            resolved_until_seq = self
                .db()
                .get_action_seq_and_timestamp(author.clone(), until_hash.clone())
                .await?
                .map(|(seq, _)| seq);
            if let Some(until_seq) = resolved_until_seq {
                if until_seq > chain_top_seq {
                    return Ok(MustGetAgentActivityResponse::UntilHashAfterChainHead(
                        until_hash.clone(),
                    ));
                }
            }
        }

        // Scan the bounded range (already ordered seq DESC, hash DESC).
        let mut activity: Vec<RegisterAgentActivity> = self
            .db()
            .get_filtered_agent_activity(author.clone(), chain_top_seq, resolved_until_seq)
            .await?
            .into_iter()
            .map(|v2| RegisterAgentActivity {
                action: to_legacy_signed_action(&v2),
                cached_entry: None,
            })
            .collect();

        // Keep only the canonical chain reachable from the chain top.
        exclude_forked_activity(&mut activity, &filter.chain_top);

        // Apply the until_timestamp filter; the bool records whether the kept set
        // has a deterministic lower-bound witness.
        let canonical_chain_precedes_until_timestamp =
            apply_timestamp_filter(&mut activity, filter.get_until_timestamp());

        // Apply the take filter.
        if let Some(take) = filter.get_take() {
            activity.truncate(take as usize);
        }

        let completeness = check_agent_activity_completeness(
            &activity,
            filter,
            canonical_chain_precedes_until_timestamp,
        );

        Ok(match completeness {
            MustGetAgentActivityCompleteness::Complete => {
                let warrants = self
                    .db()
                    .get_warrants_by_warrantee(author.clone())
                    .await?
                    .into_iter()
                    .map(warrant_row_to_signed_warrant)
                    .collect::<StateQueryResult<Vec<_>>>()?
                    .into_iter()
                    .map(WarrantOp::from)
                    .collect();
                MustGetAgentActivityResponse::Activity { activity, warrants }
            }
            MustGetAgentActivityCompleteness::IncompleteChain => {
                MustGetAgentActivityResponse::IncompleteChain
            }
            MustGetAgentActivityCompleteness::UntilHashMissing(hash) => {
                MustGetAgentActivityResponse::UntilHashMissing(hash)
            }
            MustGetAgentActivityCompleteness::UntilTimestampIndeterminate(timestamp) => {
                MustGetAgentActivityResponse::UntilTimestampIndeterminate(timestamp)
            }
        })
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
    /// The `validators` field on each
    /// [`ValidationReceipt`](holochain_types::prelude::ValidationReceipt) is
    /// populated from the supplied `validators` argument (the locally-running
    /// agents for this DNA).
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

    /// Authority-serving create-link actions for `base`: locally-validated only,
    /// each paired with its (legacy) validation status. Returns legacy types.
    pub async fn get_authority_link_creates(
        &self,
        base: &holo_hash::AnyLinkableHash,
    ) -> StateQueryResult<
        Vec<(
            holochain_zome_types::record::SignedActionHashed,
            ValidationStatus,
        )>,
    > {
        Ok(self
            .db()
            .get_authority_link_creates(base)
            .await?
            .into_iter()
            .map(|(v2, validity)| {
                (
                    holochain_zome_types::dht_v2::to_legacy_signed_action(&v2),
                    record_validity_to_status(validity),
                )
            })
            .collect())
    }

    /// Authority-serving delete-link actions targeting `base`'s links:
    /// locally-validated only, each paired with its (legacy) validation status.
    pub async fn get_authority_delete_links(
        &self,
        base: &holo_hash::AnyLinkableHash,
    ) -> StateQueryResult<
        Vec<(
            holochain_zome_types::record::SignedActionHashed,
            ValidationStatus,
        )>,
    > {
        Ok(self
            .db()
            .get_authority_delete_links(base)
            .await?
            .into_iter()
            .map(|(v2, validity)| {
                (
                    holochain_zome_types::dht_v2::to_legacy_signed_action(&v2),
                    record_validity_to_status(validity),
                )
            })
            .collect())
    }

    /// Authority-serving `StoreRecord` action for `action_hash` (locally-validated
    /// only), paired with its (legacy) validation status. Returns legacy types.
    pub async fn get_authority_store_record(
        &self,
        action_hash: &holo_hash::ActionHash,
    ) -> StateQueryResult<
        Option<(
            holochain_zome_types::record::SignedActionHashed,
            ValidationStatus,
        )>,
    > {
        Ok(self
            .db()
            .get_authority_store_record(action_hash)
            .await?
            .map(|(v2, validity)| {
                (
                    holochain_zome_types::dht_v2::to_legacy_signed_action(&v2),
                    record_validity_to_status(validity),
                )
            }))
    }

    /// Authority-serving deletes targeting record `action_hash` (locally-validated
    /// only), each paired with its (legacy) validation status.
    pub async fn get_authority_deletes_for_record(
        &self,
        action_hash: &holo_hash::ActionHash,
    ) -> StateQueryResult<
        Vec<(
            holochain_zome_types::record::SignedActionHashed,
            ValidationStatus,
        )>,
    > {
        Ok(self
            .db()
            .get_authority_deletes_for_record(action_hash)
            .await?
            .into_iter()
            .map(|(v2, validity)| {
                (
                    holochain_zome_types::dht_v2::to_legacy_signed_action(&v2),
                    record_validity_to_status(validity),
                )
            })
            .collect())
    }

    /// Authority-serving updates targeting record `action_hash` (locally-validated
    /// only), each paired with its (legacy) validation status.
    pub async fn get_authority_updates_for_record(
        &self,
        action_hash: &holo_hash::ActionHash,
    ) -> StateQueryResult<
        Vec<(
            holochain_zome_types::record::SignedActionHashed,
            ValidationStatus,
        )>,
    > {
        Ok(self
            .db()
            .get_authority_updates_for_record(action_hash)
            .await?
            .into_iter()
            .map(|(v2, validity)| {
                (
                    holochain_zome_types::dht_v2::to_legacy_signed_action(&v2),
                    record_validity_to_status(validity),
                )
            })
            .collect())
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

/// Map the v2 record validity to the legacy validation status served on the wire.
fn record_validity_to_status(v: RecordValidity) -> ValidationStatus {
    match v {
        RecordValidity::Accepted => ValidationStatus::Valid,
        RecordValidity::Rejected => ValidationStatus::Rejected,
    }
}

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

    let legacy = to_legacy_signed_action(&v2_signed);
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

    // Helper: build RecordEntry for Update ops, honouring private-entry
    // visibility. Public updates without an entry are NotStored (entry data not
    // yet local); private updates without an entry are Hidden (the entry is
    // never shared off-author). Mirrors `entry_for_action` above.
    let entry_for_update =
        |update: &holochain_zome_types::action::Update| -> StateQueryResult<RecordEntry> {
            use holochain_zome_types::entry_def::EntryVisibility;
            match decoded_entry.clone() {
                Some(entry) => Ok(RecordEntry::Present(entry)),
                None => match update.entry_type.visibility() {
                    EntryVisibility::Private => Ok(RecordEntry::Hidden),
                    EntryVisibility::Public => Ok(RecordEntry::NotStored),
                },
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

/// Reconstruct a legacy [`SignedWarrant`] from an integrated `WarrantRow`.
fn warrant_row_to_signed_warrant(
    row: holochain_data::models::dht::WarrantRow,
) -> StateQueryResult<SignedWarrant> {
    use holochain_types::prelude::{Signature, Timestamp};
    use holochain_zome_types::warrant::{SignedWarrant, Warrant, WarrantProof};

    let proof: WarrantProof = holochain_serialized_bytes::decode(&row.proof)?;
    let author = holo_hash::AgentPubKey::from_raw_36(row.author);
    let warrantee = holo_hash::AgentPubKey::from_raw_36(row.warrantee);
    let timestamp = Timestamp::from_micros(row.timestamp);
    let warrant = Warrant::new(proof, author, timestamp, warrantee);
    let sig: [u8; 64] = row.signature.as_slice().try_into().map_err(|_| {
        crate::query::StateQueryError::Other(format!(
            "warrant signature column has {} bytes, expected 64",
            row.signature.len()
        ))
    })?;
    Ok(SignedWarrant::new(warrant, Signature::from(sig)))
}

/// Highest observed sequence number across the valid and rejected lists, each
/// assumed sorted ascending by sequence. Mirrors the legacy authority: the hash
/// list holds the last valid and/or last rejected action, which coincide only
/// when both share the top sequence.
fn compute_highest_observed<T: ActionSequenceAndHash>(
    valid: &[T],
    rejected: &[T],
) -> Option<HighestObserved> {
    let mut highest_observed: Option<u32> = None;
    let mut hashes = Vec::new();
    let mut check_highest = |seq: u32, hash: &holo_hash::ActionHash| {
        if let Some(last) = highest_observed.as_mut() {
            match seq.cmp(last) {
                std::cmp::Ordering::Less => {}
                std::cmp::Ordering::Equal => hashes.push(hash.clone()),
                std::cmp::Ordering::Greater => {
                    hashes.clear();
                    hashes.push(hash.clone());
                    *last = seq;
                }
            }
        } else {
            highest_observed = Some(seq);
            hashes.push(hash.clone());
        }
    };
    if let Some(v) = valid.last() {
        check_highest(v.action_seq(), v.address());
    }
    if let Some(r) = rejected.last() {
        check_highest(r.action_seq(), r.address());
    }
    highest_observed.map(|action_seq| HighestObserved {
        action_seq,
        hash: hashes,
    })
}

/// Compute the [`ChainStatus`] from the valid and rejected lists, returning the
/// (sorted) lists alongside. A fork is two valid actions at the same sequence.
fn compute_chain_status<T: ActionSequenceAndHash>(
    valid: impl Iterator<Item = T>,
    rejected: impl Iterator<Item = T>,
) -> (ChainStatus, Vec<T>, Vec<T>) {
    let mut valid: Vec<_> = valid.collect();
    let mut rejected: Vec<_> = rejected.collect();
    valid.sort_unstable_by_key(|a| a.action_seq());
    rejected.sort_unstable_by_key(|a| a.action_seq());

    let mut valid_out: Vec<T> = Vec::with_capacity(valid.len());
    let mut status = None;
    for current in valid {
        if status.is_none() {
            let fork = valid_out.last().and_then(|v: &T| {
                if current.action_seq() == v.action_seq() {
                    Some(v)
                } else {
                    None
                }
            });
            if let Some(fork) = fork {
                status = Some(ChainStatus::Forked(ChainFork {
                    fork_seq: current.action_seq(),
                    first_action: current.address().clone(),
                    second_action: fork.address().clone(),
                }));
            }
        }
        valid_out.push(current);
    }

    let status = status.unwrap_or_else(|| match (valid_out.last(), rejected.first()) {
        (None, None) => ChainStatus::Empty,
        (Some(v), None) => ChainStatus::Valid(ChainHead {
            action_seq: v.action_seq(),
            hash: v.address().clone(),
        }),
        (None, Some(r)) | (Some(_), Some(r)) => ChainStatus::Invalid(ChainHead {
            action_seq: r.action_seq(),
            hash: r.address().clone(),
        }),
    });

    (status, valid_out, rejected)
}

/// An intermediary type describing the completeness of a `must_get` result.
enum MustGetAgentActivityCompleteness {
    Complete,
    IncompleteChain,
    UntilHashMissing(holo_hash::ActionHash),
    UntilTimestampIndeterminate(holochain_types::prelude::Timestamp),
}

/// Remove forked actions by walking the chain backwards from `chain_top`.
/// Input must already be sorted by action seq descending.
fn exclude_forked_activity(
    activity: &mut Vec<RegisterAgentActivity>,
    chain_top: &holo_hash::ActionHash,
) {
    if activity.is_empty() {
        return;
    }
    let chain_hashes = collect_canonical_chain_hashes(activity, chain_top);
    activity.retain(|a| chain_hashes.contains(&a.action.hashed.hash));
}

/// Walk the chain from `chain_top` backwards (via `prev_action`), collecting the
/// reachable action hashes. Input must already be sorted by action seq descending.
fn collect_canonical_chain_hashes(
    activity: &[RegisterAgentActivity],
    chain_top: &holo_hash::ActionHash,
) -> HashSet<holo_hash::ActionHash> {
    let index_by_hash: HashMap<holo_hash::ActionHash, usize> = activity
        .iter()
        .enumerate()
        .map(|(i, a)| (a.action.hashed.hash.clone(), i))
        .collect();

    let mut chain_hashes: HashSet<holo_hash::ActionHash> = HashSet::new();

    let Some(&walk_index) = index_by_hash.get(chain_top) else {
        return chain_hashes;
    };

    let mut walk_index = walk_index;
    for _ in 0..activity.len() {
        let current = &activity[walk_index];
        let current_hash = current.action.hashed.hash.clone();
        if !chain_hashes.insert(current_hash) {
            break;
        }
        let Some(prev_hash) = current.action.prev_hash() else {
            break;
        };
        let Some(&prev_index) = index_by_hash.get(prev_hash) else {
            break;
        };
        walk_index = prev_index;
    }

    chain_hashes
}

/// Apply the `until_timestamp` filter. Returns `true` when the kept set has a
/// deterministic lower-bound witness (an action with timestamp < `until_timestamp`).
fn apply_timestamp_filter(
    activity: &mut Vec<RegisterAgentActivity>,
    until_timestamp: Option<holochain_types::prelude::Timestamp>,
) -> bool {
    match until_timestamp {
        None => false,
        Some(until_ts) => {
            let precedes_boundary = activity
                .last()
                .map(|a| a.action.action().timestamp() < until_ts)
                .unwrap_or(false);
            activity.retain(|a| a.action.action().timestamp() >= until_ts);
            precedes_boundary
        }
    }
}

/// Decide whether `activity` is a complete response for `filter`.
fn check_agent_activity_completeness(
    activity: &[RegisterAgentActivity],
    filter: &ChainFilter,
    canonical_chain_precedes_until_timestamp: bool,
) -> MustGetAgentActivityCompleteness {
    let has_gap = activity
        .windows(2)
        .any(|w| w[0].action.seq() != w[1].action.seq() + 1);
    let reaches_genesis = activity
        .last()
        .map(|last| last.action.seq() == 0)
        .unwrap_or(false);

    match &filter.limit_conditions {
        LimitConditions::ToGenesis => {
            if has_gap || !reaches_genesis {
                MustGetAgentActivityCompleteness::IncompleteChain
            } else {
                MustGetAgentActivityCompleteness::Complete
            }
        }
        LimitConditions::UntilHash(until_hash) => {
            if !activity.iter().any(|a| &a.action.hashed.hash == until_hash) {
                MustGetAgentActivityCompleteness::UntilHashMissing(until_hash.clone())
            } else if has_gap {
                MustGetAgentActivityCompleteness::IncompleteChain
            } else {
                MustGetAgentActivityCompleteness::Complete
            }
        }
        LimitConditions::UntilTimestamp(until_timestamp) => {
            let any_satisfies_timestamp = activity
                .iter()
                .any(|a| a.action.action().timestamp() >= *until_timestamp);
            if !any_satisfies_timestamp
                || (!reaches_genesis && !canonical_chain_precedes_until_timestamp)
            {
                MustGetAgentActivityCompleteness::UntilTimestampIndeterminate(*until_timestamp)
            } else if has_gap {
                MustGetAgentActivityCompleteness::IncompleteChain
            } else {
                MustGetAgentActivityCompleteness::Complete
            }
        }
        LimitConditions::Take(take) => {
            let take = *take as usize;
            if activity.len() >= take {
                if has_gap {
                    MustGetAgentActivityCompleteness::IncompleteChain
                } else {
                    MustGetAgentActivityCompleteness::Complete
                }
            } else if has_gap || !reaches_genesis {
                MustGetAgentActivityCompleteness::IncompleteChain
            } else {
                MustGetAgentActivityCompleteness::Complete
            }
        }
    }
}

/// Assemble the legacy [`AgentActivityResponse`] from the classified lists.
fn build_agent_activity_response<T>(
    agent: holo_hash::AgentPubKey,
    valid: Vec<T>,
    rejected: Vec<T>,
    warrants: Vec<SignedWarrant>,
    filter: &ChainQueryFilter,
    options: &crate::dht_store::GetAgentActivityOptions,
) -> AgentActivityResponse
where
    T: ActionHashedContainer + Clone,
    Vec<T>: ChainItemsSource,
{
    // `compute_chain_status` returns the lists sorted ascending by sequence, so
    // computing `highest_observed` from its output makes the "sorted input"
    // precondition structurally guaranteed rather than relying on the caller's
    // SQL ordering.
    let (status, valid, rejected) = compute_chain_status(valid.into_iter(), rejected.into_iter());
    let highest_observed = compute_highest_observed(&valid, &rejected);

    let valid_activity = if options.include_valid_activity {
        filter.filter_actions(valid).to_chain_items()
    } else {
        ChainItems::NotRequested
    };
    let rejected_activity = if options.include_rejected_activity {
        filter.filter_actions(rejected).to_chain_items()
    } else {
        ChainItems::NotRequested
    };
    // Warrant gating is owned entirely by the caller, which passes an empty
    // `warrants` when `include_warrants` is false; this fn passes it through.
    AgentActivityResponse {
        agent,
        valid_activity,
        rejected_activity,
        warrants,
        status,
        highest_observed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dht_store::{AppOutcome, GetAgentActivityOptions, SysOutcome};
    use holo_hash::{ActionHash, AgentPubKey, DhtOpHash, EntryHash, HoloHashed};
    use holochain_data::kind::Dht;
    use holochain_data::DbWrite;
    use holochain_types::chain::ChainItem;
    use holochain_types::dht_op::{ChainOp, DhtOp, DhtOpHashed};
    use holochain_types::prelude::MustGetAgentActivityResponse;
    use holochain_types::prelude::Signature;
    use holochain_types::prelude::Timestamp;
    use holochain_zome_types::action::{Action, Create, EntryType};
    use holochain_zome_types::chain::ChainFilter;
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
        let exists = store.as_read().op_exists(&unknown).await.unwrap();
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

        let exists = store.as_read().op_exists(&hash).await.unwrap();
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

        let pending = store
            .as_read()
            .ops_pending_sys_validation(1_000)
            .await
            .unwrap();
        let hashes: Vec<_> = pending.iter().map(|o| o.as_hash().clone()).collect();
        assert!(hashes.contains(&hash));
    }

    #[tokio::test]
    async fn ops_pending_sys_validation_excludes_completed() {
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

        let pending = store
            .as_read()
            .ops_pending_sys_validation(1_000)
            .await
            .unwrap();
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

        let pending = store.as_read().ops_pending_sys_validation(2).await.unwrap();
        assert_eq!(pending.len(), 2);
    }

    #[tokio::test]
    async fn ops_pending_app_validation_returns_sys_validated_chain_op() {
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

        let pending = store
            .as_read()
            .ops_pending_app_validation(1_000)
            .await
            .unwrap();
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

        let pending = store
            .as_read()
            .ops_pending_app_validation(1_000)
            .await
            .unwrap();
        let hashes: Vec<_> = pending.iter().map(|o| o.as_hash().clone()).collect();
        assert!(
            !hashes.contains(&hash),
            "op not yet sys-validated should not appear"
        );
    }

    #[tokio::test]
    async fn ops_pending_app_validation_excludes_app_validated() {
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

        let pending = store
            .as_read()
            .ops_pending_app_validation(1_000)
            .await
            .unwrap();
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

        let result = store.as_read().find_fork_for_action(&action).await.unwrap();
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

        let result = store
            .as_read()
            .find_fork_for_action(&action_b)
            .await
            .unwrap();
        let (got_hash, got_sig) = result.expect("fork should be detected");
        assert_eq!(got_hash, expected_hash, "sibling hash should match op_a");
        assert_eq!(got_sig, expected_sig, "sibling signature should match op_a");
    }

    #[tokio::test]
    async fn pending_validation_receipts_returns_integrated_require_receipt_ops() {
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
            .as_read()
            .pending_validation_receipts(validators.clone())
            .await
            .unwrap();

        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].0.dht_op_hash, hash);
        assert_eq!(receipts[0].1, author);
        assert_eq!(receipts[0].0.validators, validators);
    }

    async fn integrate_activity(
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

    #[tokio::test]
    async fn get_agent_activity_valid_chain_hashes() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let author = AgentPubKey::from_raw_36(vec![42u8; 36]);
        let prev = ActionHash::from_raw_36(vec![0u8; 36]);

        integrate_activity(
            &store,
            make_fork_op(&author, &prev, 0, 1),
            AppOutcome::Accepted,
            10,
        )
        .await;
        integrate_activity(
            &store,
            make_fork_op(&author, &prev, 1, 2),
            AppOutcome::Accepted,
            11,
        )
        .await;

        let opts = GetAgentActivityOptions {
            include_valid_activity: true,
            include_rejected_activity: true,
            include_warrants: false,
            include_full_records: false,
        };
        let resp: AgentActivityResponse = store
            .as_read()
            .get_agent_activity(&author, &ChainQueryFilter::new(), &opts)
            .await
            .unwrap();

        assert_eq!(resp.agent, author);
        match resp.valid_activity {
            ChainItems::Hashes(h) => {
                assert_eq!(h.len(), 2);
                assert_eq!(h[0].0, 0);
                assert_eq!(h[1].0, 1);
            }
            other => panic!("expected Hashes, got {other:?}"),
        }
        assert!(matches!(resp.rejected_activity, ChainItems::Hashes(ref h) if h.is_empty()));
        assert!(matches!(resp.status, ChainStatus::Valid(ref head) if head.action_seq == 1));
        let ho = resp.highest_observed.expect("highest observed");
        assert_eq!(ho.action_seq, 1);
    }

    #[tokio::test]
    async fn get_agent_activity_rejected_marks_invalid() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let author = AgentPubKey::from_raw_36(vec![7u8; 36]);
        let prev = ActionHash::from_raw_36(vec![0u8; 36]);

        integrate_activity(
            &store,
            make_fork_op(&author, &prev, 0, 1),
            AppOutcome::Accepted,
            10,
        )
        .await;
        integrate_activity(
            &store,
            make_fork_op(&author, &prev, 1, 2),
            AppOutcome::Rejected,
            11,
        )
        .await;

        let opts = GetAgentActivityOptions {
            include_valid_activity: true,
            include_rejected_activity: true,
            include_warrants: false,
            include_full_records: false,
        };
        let resp = store
            .as_read()
            .get_agent_activity(&author, &ChainQueryFilter::new(), &opts)
            .await
            .unwrap();

        assert!(matches!(resp.valid_activity, ChainItems::Hashes(ref h) if h.len() == 1));
        match resp.rejected_activity {
            ChainItems::Hashes(h) => {
                assert_eq!(h.len(), 1);
                assert_eq!(h[0].0, 1);
            }
            other => panic!("expected Hashes, got {other:?}"),
        }
        assert!(matches!(resp.status, ChainStatus::Invalid(ref head) if head.action_seq == 1));
    }

    #[tokio::test]
    async fn get_agent_activity_detects_fork() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let author = AgentPubKey::from_raw_36(vec![9u8; 36]);
        let prev = ActionHash::from_raw_36(vec![0u8; 36]);

        integrate_activity(
            &store,
            make_fork_op(&author, &prev, 0, 1),
            AppOutcome::Accepted,
            10,
        )
        .await;
        // Two valid actions both at seq 1 -> fork.
        integrate_activity(
            &store,
            make_fork_op(&author, &prev, 1, 2),
            AppOutcome::Accepted,
            11,
        )
        .await;
        integrate_activity(
            &store,
            make_fork_op(&author, &prev, 1, 3),
            AppOutcome::Accepted,
            12,
        )
        .await;

        let opts = GetAgentActivityOptions {
            include_valid_activity: true,
            include_rejected_activity: false,
            include_warrants: false,
            include_full_records: false,
        };
        let resp = store
            .as_read()
            .get_agent_activity(&author, &ChainQueryFilter::new(), &opts)
            .await
            .unwrap();

        // The meaningful signal is fork detection: two valid actions at the
        // same seq yield Forked. (highest_observed mirrors the legacy authority
        // and reports the top sequence, not necessarily every tip's hash.)
        match resp.status {
            ChainStatus::Forked(fork) => assert_eq!(fork.fork_seq, 1),
            other => panic!("expected Forked, got {other:?}"),
        }
        let ho = resp.highest_observed.expect("highest observed");
        assert_eq!(ho.action_seq, 1);
    }

    #[tokio::test]
    async fn get_agent_activity_full_returns_records() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let author = AgentPubKey::from_raw_36(vec![5u8; 36]);
        let prev = ActionHash::from_raw_36(vec![0u8; 36]);

        integrate_activity(
            &store,
            make_fork_op(&author, &prev, 0, 1),
            AppOutcome::Accepted,
            10,
        )
        .await;

        let opts = GetAgentActivityOptions {
            include_valid_activity: true,
            include_rejected_activity: false,
            include_warrants: false,
            include_full_records: true,
        };
        let resp = store
            .as_read()
            .get_agent_activity(&author, &ChainQueryFilter::new(), &opts)
            .await
            .unwrap();

        match resp.valid_activity {
            ChainItems::Full(records) => assert_eq!(records.len(), 1),
            other => panic!("expected Full, got {other:?}"),
        }
    }

    fn make_warrant_for(warrantee: &AgentPubKey, seed: u8) -> DhtOpHashed {
        // `Signature` and `Timestamp` are already in scope from the test
        // module's top-level `use` lines; do not re-import them here.
        use holochain_types::warrant::WarrantOp;
        use holochain_zome_types::op::ChainOpType;
        use holochain_zome_types::prelude::{
            ChainIntegrityWarrant, SignedWarrant, Warrant, WarrantProof,
        };

        let action_author = AgentPubKey::from_raw_36(vec![seed; 36]);
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
                warrantee.clone(),
            ),
            Signature::from([seed.wrapping_add(1); 64]),
        );
        DhtOpHashed::from_content_sync(DhtOp::WarrantOp(Box::new(WarrantOp::from(warrant))))
    }

    #[tokio::test]
    async fn get_agent_activity_attaches_warrants_when_requested() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let author = AgentPubKey::from_raw_36(vec![21u8; 36]);
        let prev = ActionHash::from_raw_36(vec![0u8; 36]);

        integrate_activity(
            &store,
            make_fork_op(&author, &prev, 0, 1),
            AppOutcome::Accepted,
            10,
        )
        .await;

        // Integrate a warrant whose warrantee is this author.
        let warrant = make_warrant_for(&author, 30);
        let wh = warrant.as_hash().clone();
        store.record_incoming_ops(vec![warrant]).await.unwrap();
        store
            .record_warrant_sys_validation_outcomes(vec![(wh, SysOutcome::Accepted)])
            .await
            .unwrap();
        store
            .integrate_ready_ops(Timestamp::from_micros(20))
            .await
            .unwrap();

        // include_warrants = true -> attached.
        let with = GetAgentActivityOptions {
            include_valid_activity: true,
            include_rejected_activity: false,
            include_warrants: true,
            include_full_records: false,
        };
        let resp = store
            .as_read()
            .get_agent_activity(&author, &ChainQueryFilter::new(), &with)
            .await
            .unwrap();
        assert_eq!(resp.warrants.len(), 1);

        // include_warrants = false -> empty, and valid not requested.
        let without = GetAgentActivityOptions {
            include_valid_activity: false,
            include_rejected_activity: false,
            include_warrants: false,
            include_full_records: false,
        };
        let resp = store
            .as_read()
            .get_agent_activity(&author, &ChainQueryFilter::new(), &without)
            .await
            .unwrap();
        assert!(resp.warrants.is_empty());
        assert!(matches!(resp.valid_activity, ChainItems::NotRequested));
    }

    /// Build a linked chain of `len` `RegisterAgentActivity` ops for `author`
    /// (seq 0..len, each `prev_action` = the previous action's hash). Returns the
    /// ops and the action hashes (index = seq).
    fn make_activity_chain(author: &AgentPubKey, len: u32) -> (Vec<DhtOpHashed>, Vec<ActionHash>) {
        let mut ops = Vec::new();
        let mut hashes = Vec::new();
        let mut prev = ActionHash::from_raw_36(vec![0u8; 36]);
        for seq in 0..len {
            let action = Action::Create(Create {
                author: author.clone(),
                timestamp: Timestamp::from_micros((seq as i64 + 1) * 1000),
                action_seq: seq,
                prev_action: prev.clone(),
                entry_type: EntryType::App(AppEntryDef::new(
                    0.into(),
                    0.into(),
                    EntryVisibility::Public,
                )),
                entry_hash: EntryHash::from_raw_36(vec![(seq as u8).wrapping_add(100); 36]),
                weight: Default::default(),
            });
            let action_hash = holo_hash::ActionHash::with_data_sync(&action);
            let op = DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(
                ChainOp::RegisterAgentActivity(Signature::from([seq as u8; 64]), action),
            )));
            prev = action_hash.clone();
            hashes.push(action_hash);
            ops.push(op);
        }
        (ops, hashes)
    }

    #[tokio::test]
    async fn must_get_agent_activity_to_genesis_complete() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let author = AgentPubKey::from_raw_36(vec![71u8; 36]);
        let (ops, hashes) = make_activity_chain(&author, 3);
        for (i, op) in ops.into_iter().enumerate() {
            integrate_activity(&store, op, AppOutcome::Accepted, 10 + i as i64).await;
        }

        // A warrant for this author should be attached to a Complete response.
        let warrant = make_warrant_for(&author, 40);
        let wh = warrant.as_hash().clone();
        store.record_incoming_ops(vec![warrant]).await.unwrap();
        store
            .record_warrant_sys_validation_outcomes(vec![(wh, SysOutcome::Accepted)])
            .await
            .unwrap();
        store
            .integrate_ready_ops(Timestamp::from_micros(99))
            .await
            .unwrap();

        let filter = ChainFilter::new(hashes[2].clone());
        let resp = store
            .as_read()
            .must_get_agent_activity(&author, &filter)
            .await
            .unwrap();

        match resp {
            MustGetAgentActivityResponse::Activity { activity, warrants } => {
                assert_eq!(activity.len(), 3);
                // Returned newest-first (seq DESC).
                assert_eq!(activity[0].action.seq(), 2);
                assert_eq!(activity[2].action.seq(), 0);
                assert_eq!(warrants.len(), 1);
            }
            other => panic!("expected Activity, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn must_get_agent_activity_chain_top_not_found() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let author = AgentPubKey::from_raw_36(vec![72u8; 36]);
        let (ops, _hashes) = make_activity_chain(&author, 2);
        for (i, op) in ops.into_iter().enumerate() {
            integrate_activity(&store, op, AppOutcome::Accepted, 10 + i as i64).await;
        }
        let unknown = ActionHash::from_raw_36(vec![88u8; 36]);
        let resp = store
            .as_read()
            .must_get_agent_activity(&author, &ChainFilter::new(unknown.clone()))
            .await
            .unwrap();
        assert!(matches!(
            resp,
            MustGetAgentActivityResponse::ChainTopNotFound(h) if h == unknown
        ));
    }

    #[tokio::test]
    async fn must_get_agent_activity_until_hash_complete() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let author = AgentPubKey::from_raw_36(vec![73u8; 36]);
        let (ops, hashes) = make_activity_chain(&author, 3);
        for (i, op) in ops.into_iter().enumerate() {
            integrate_activity(&store, op, AppOutcome::Accepted, 10 + i as i64).await;
        }
        // until = seq 1: expect seq 2 and 1, Complete.
        let filter = ChainFilter::until_hash(hashes[2].clone(), hashes[1].clone());
        let resp = store
            .as_read()
            .must_get_agent_activity(&author, &filter)
            .await
            .unwrap();
        match resp {
            MustGetAgentActivityResponse::Activity { activity, .. } => {
                assert_eq!(activity.len(), 2);
                assert_eq!(activity[0].action.seq(), 2);
                assert_eq!(activity[1].action.seq(), 1);
            }
            other => panic!("expected Activity, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn must_get_agent_activity_until_hash_missing() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let author = AgentPubKey::from_raw_36(vec![74u8; 36]);
        let (ops, hashes) = make_activity_chain(&author, 2);
        for (i, op) in ops.into_iter().enumerate() {
            integrate_activity(&store, op, AppOutcome::Accepted, 10 + i as i64).await;
        }
        // until_hash references an action not in this chain.
        let missing = ActionHash::from_raw_36(vec![77u8; 36]);
        let filter = ChainFilter::until_hash(hashes[1].clone(), missing.clone());
        let resp = store
            .as_read()
            .must_get_agent_activity(&author, &filter)
            .await
            .unwrap();
        assert!(matches!(
            resp,
            MustGetAgentActivityResponse::UntilHashMissing(h) if h == missing
        ));
    }

    #[tokio::test]
    async fn must_get_agent_activity_take_complete() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let author = AgentPubKey::from_raw_36(vec![75u8; 36]);
        let (ops, hashes) = make_activity_chain(&author, 3);
        for (i, op) in ops.into_iter().enumerate() {
            integrate_activity(&store, op, AppOutcome::Accepted, 10 + i as i64).await;
        }
        let filter = ChainFilter::take(hashes[2].clone(), 2);
        let resp = store
            .as_read()
            .must_get_agent_activity(&author, &filter)
            .await
            .unwrap();
        match resp {
            MustGetAgentActivityResponse::Activity { activity, .. } => {
                assert_eq!(activity.len(), 2);
                assert_eq!(activity[0].action.seq(), 2);
                assert_eq!(activity[1].action.seq(), 1);
            }
            other => panic!("expected Activity, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn must_get_agent_activity_take_zero_errors() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let author = AgentPubKey::from_raw_36(vec![76u8; 36]);
        let (ops, hashes) = make_activity_chain(&author, 1);
        for (i, op) in ops.into_iter().enumerate() {
            integrate_activity(&store, op, AppOutcome::Accepted, 10 + i as i64).await;
        }
        let filter = ChainFilter::take(hashes[0].clone(), 0);
        let result = store
            .as_read()
            .must_get_agent_activity(&author, &filter)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn must_get_agent_activity_gap_is_incomplete() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let author = AgentPubKey::from_raw_36(vec![78u8; 36]);
        let (ops, hashes) = make_activity_chain(&author, 3);
        // Integrate seq 0 and seq 2, skip seq 1 -> the chain top cannot reach genesis.
        integrate_activity(&store, ops[0].clone(), AppOutcome::Accepted, 10).await;
        integrate_activity(&store, ops[2].clone(), AppOutcome::Accepted, 12).await;
        let filter = ChainFilter::new(hashes[2].clone());
        let resp = store
            .as_read()
            .must_get_agent_activity(&author, &filter)
            .await
            .unwrap();
        assert!(matches!(
            resp,
            MustGetAgentActivityResponse::IncompleteChain
        ));
    }

    #[tokio::test]
    async fn pending_validation_receipts_excludes_ops_without_require_receipt() {
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

        let receipts = store
            .as_read()
            .pending_validation_receipts(vec![])
            .await
            .unwrap();
        assert!(
            receipts.iter().all(|(r, _)| r.dht_op_hash != hash),
            "op with cleared require_receipt should not appear"
        );
    }
}
