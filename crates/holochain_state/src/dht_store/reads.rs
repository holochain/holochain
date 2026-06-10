//! Read operations on the per-DNA DHT store.
//!
//! Methods on [`DhtStoreRead`] expose domain-meaningful reads that delegate to
//! `holochain_data`'s `DbRead<Dht>` primitives and return values in terms of
//! the project's existing domain types. The parent module holds the
//! corresponding write operations.

use super::DhtStore;
use crate::prelude::ActionSequenceAndHash;
use crate::query::StateQueryResult;
use crate::scratch::SyncScratch;
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
            Some(entry_hash) if private_entry_visible_to(&action, author) => {
                match self.db().get_entry(entry_hash.clone(), author).await? {
                    Some(entry) => Some(entry),
                    // A public entry referenced but unavailable means "no
                    // record"; an absent private entry is simply `Hidden`.
                    None if action_entry_is_private(&action) => None,
                    None => return Ok(None),
                }
            }
            // A private entry is `Hidden` unless the caller is the author —
            // never attach another agent's private entry, even if the caller
            // happens to hold a same-hash private entry of their own.
            Some(_) => None,
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

    /// Retrieve the signed action for `hash`, checking the store first and
    /// falling back to the in-memory scratch on a miss.
    ///
    /// Use this on the **requester** path only. Authority handlers must never
    /// see scratch data — use [`retrieve_action`](Self::retrieve_action) there.
    pub async fn retrieve_action_with_scratch(
        &self,
        hash: &holo_hash::ActionHash,
        scratch: &SyncScratch,
    ) -> StateQueryResult<Option<holochain_zome_types::record::SignedActionHashed>> {
        if let Some(sah) = self.retrieve_action(hash).await? {
            return Ok(Some(sah));
        }
        scratch_action(scratch, hash)
    }

    /// Retrieve the entry for `hash`, checking the store first and falling back
    /// to the in-memory scratch on a miss.
    ///
    /// `author` visibility follows the same rule as
    /// [`retrieve_entry`](Self::retrieve_entry): `None` returns public entries
    /// only from the store; entries in the scratch are always returned because
    /// the scratch is by definition this agent's own authored data.
    ///
    /// Use this on the **requester** path only.
    pub async fn retrieve_entry_with_scratch(
        &self,
        hash: &holo_hash::EntryHash,
        author: Option<&holo_hash::AgentPubKey>,
        scratch: &SyncScratch,
    ) -> StateQueryResult<Option<holochain_types::prelude::Entry>> {
        if let Some(entry) = self.retrieve_entry(hash, author).await? {
            return Ok(Some(entry));
        }
        scratch_entry(scratch, hash)
    }

    /// Retrieve the complete record (action + entry, if any) for `hash`,
    /// resolving both the action and the entry from store-or-scratch.
    ///
    /// Preserves the [`retrieve_record`](Self::retrieve_record) contract: if
    /// the action references an entry that is unavailable in both the store
    /// *and* the scratch, returns `None`.
    ///
    /// Use this on the **requester** path only.
    pub async fn retrieve_record_with_scratch(
        &self,
        hash: &holo_hash::ActionHash,
        author: Option<&holo_hash::AgentPubKey>,
        scratch: &SyncScratch,
    ) -> StateQueryResult<Option<holochain_zome_types::record::Record>> {
        // Resolve the action from store, then scratch.
        let action = match self.retrieve_action(hash).await? {
            Some(sah) => sah,
            None => match scratch_action(scratch, hash)? {
                Some(sah) => sah,
                None => return Ok(None),
            },
        };

        // Resolve the entry (if the action references one). A private entry is
        // only attached for its own author — see `private_entry_visible_to`.
        let entry = match action.action().entry_hash() {
            Some(entry_hash) if private_entry_visible_to(&action, author) => {
                // Try the store first, then the scratch (scratch entries are
                // this agent's own data; the visibility guard above already
                // ensures we only reach here for the author of a private entry).
                match self.retrieve_entry(entry_hash, author).await? {
                    Some(e) => Some(e),
                    None => match scratch_entry(scratch, entry_hash)? {
                        Some(e) => Some(e),
                        // Unavailable everywhere: a public entry means "no
                        // record"; an absent private entry is `Hidden`.
                        None if action_entry_is_private(&action) => None,
                        None => return Ok(None),
                    },
                }
            }
            // Private entry, caller is not the author -> Hidden.
            Some(_) => None,
            None => None,
        };

        Ok(Some(holochain_zome_types::record::Record::new(
            action, entry,
        )))
    }

    /// Retrieve the record for `hash` only if it is still live, checking the
    /// store **and** the in-memory scratch for delete tombstones.
    ///
    /// Returns `None` when:
    /// - the store holds an integrated `Delete` targeting `hash`, **or**
    /// - the scratch holds a `Delete` action whose `deletes_address == hash`.
    ///
    /// Otherwise delegates to [`retrieve_record_with_scratch`](Self::retrieve_record_with_scratch)
    /// to resolve the action and entry from store-or-scratch.
    ///
    /// Use this on the **requester** path only. Authority handlers must never
    /// see scratch data — use [`get_live_record`](Self::get_live_record) there.
    pub async fn get_live_record_with_scratch(
        &self,
        hash: &holo_hash::ActionHash,
        author: Option<&holo_hash::AgentPubKey>,
        scratch: &SyncScratch,
    ) -> StateQueryResult<Option<holochain_zome_types::record::Record>> {
        // An integrated delete in the store is decisive — record is dead.
        if !self
            .db()
            .get_deleted_records(hash.clone())
            .await?
            .is_empty()
        {
            return Ok(None);
        }
        // A pending delete in the scratch also kills the record.
        if scratch_delete_targets(scratch)?.contains(hash) {
            return Ok(None);
        }
        self.retrieve_record_with_scratch(hash, author, scratch)
            .await
    }

    /// The live `Record` for an entry, consulting both the store and the
    /// in-memory scratch for create candidates and delete tombstones.
    ///
    /// Live creates are the union of:
    /// - the store's validated, integrated, undeleted `StoreEntry` creates
    ///   (from [`get_live_entry_creates`](holochain_data::DbRead::get_live_entry_creates)), and
    /// - scratch `Create`/`Update` actions whose `entry_hash() == entry_hash`
    ///   and that are not targeted by a scratch `Delete`.
    ///
    /// Among these, the authored-or-first preference from
    /// [`get_live_entry`](Self::get_live_entry) is preserved: if `author` is
    /// `Some`, the first action by that author wins; otherwise the first store
    /// create (by integration order) is chosen, then the first scratch create.
    ///
    /// The entry is resolved via store-or-scratch (same as
    /// [`retrieve_entry_with_scratch`](Self::retrieve_entry_with_scratch)).
    ///
    /// Use this on the **requester** path only.
    pub async fn get_live_entry_with_scratch(
        &self,
        entry_hash: &holo_hash::EntryHash,
        author: Option<&holo_hash::AgentPubKey>,
        scratch: &SyncScratch,
    ) -> StateQueryResult<Option<holochain_zome_types::record::Record>> {
        // Collect validated, integrated store creates, then drop any tombstoned
        // by a pending scratch `Delete` (the store read only excludes integrated
        // store deletes, so the scratch-delete exclusion is applied here).
        let scratch_deleted = scratch_delete_targets(scratch)?;
        let store_creates: Vec<_> = self
            .db()
            .get_live_entry_creates(entry_hash, author)
            .await?
            .into_iter()
            .filter(|sah| !scratch_deleted.contains(sah.as_hash()))
            .collect();

        // Collect live scratch creates/updates for this entry.
        let scratch_creates: Vec<holochain_zome_types::record::SignedActionHashed> =
            scratch_live_entry_creates(scratch, entry_hash)?;

        // Author-preference: prefer a create by `author`; failing that, first
        // store create (integration order), then first scratch create.
        let chosen = match author {
            Some(a) => {
                // Search store creates first, then scratch creates.
                let authored = store_creates
                    .iter()
                    .find(|sah| &sah.hashed.content.header.author == a)
                    .map(holochain_zome_types::dht_v2::to_legacy_signed_action);
                let authored = authored.or_else(|| {
                    scratch_creates
                        .iter()
                        .find(|sah| sah.action().author() == a)
                        .cloned()
                });
                authored
                    .or_else(|| {
                        store_creates
                            .into_iter()
                            .next()
                            .map(|sah| holochain_zome_types::dht_v2::to_legacy_signed_action(&sah))
                    })
                    .or_else(|| scratch_creates.into_iter().next())
            }
            None => store_creates
                .into_iter()
                .next()
                .map(|sah| holochain_zome_types::dht_v2::to_legacy_signed_action(&sah))
                .or_else(|| scratch_creates.into_iter().next()),
        };

        let Some(action) = chosen else {
            return Ok(None);
        };

        // Resolve the entry from store-or-scratch.
        let entry = self
            .retrieve_entry_with_scratch(entry_hash, author, scratch)
            .await?;

        Ok(Some(holochain_zome_types::record::Record::new(
            action, entry,
        )))
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

    /// Assemble the [`RecordDetails`] for `hash`, overlaying the in-memory
    /// scratch for deletes and updates.
    ///
    /// # Contract
    ///
    /// Validation status is derived exclusively from the integrated `StoreRecord`
    /// op in the store. A scratch-only action (one that has been authored locally
    /// but not yet committed and published) has no such op, so this method returns
    /// `None` in that case. The record's liveness is therefore always grounded in
    /// a peer-visible, validated store entry.
    ///
    /// Deletes and updates are the **union** of:
    /// - store-integrated `RegisterDeletedBy` / `RegisterUpdatedRecord` actions, and
    /// - scratch `Delete`/`Update` actions targeting `hash`.
    ///
    /// Use this on the **requester** path only. Authority handlers must never
    /// see scratch data — use [`get_record_details`](Self::get_record_details) there.
    pub async fn get_record_details_with_scratch(
        &self,
        hash: &holo_hash::ActionHash,
        author: Option<&holo_hash::AgentPubKey>,
        scratch: &SyncScratch,
    ) -> StateQueryResult<Option<holochain_zome_types::metadata::RecordDetails>> {
        use holochain_zome_types::op::ChainOpType;

        // Validation status and the existence gate require an integrated
        // StoreRecord op. A scratch-only record has no such op.
        let ops = self.db().get_chain_ops_for_action(hash.clone()).await?;
        let Some(store_op) = ops
            .iter()
            .find(|r| ChainOpType::try_from(r.op_type) == Ok(ChainOpType::StoreRecord))
        else {
            return Ok(None);
        };
        let validation_status = match RecordValidity::try_from(store_op.validation_status) {
            Ok(RecordValidity::Accepted) => ValidationStatus::Valid,
            Ok(RecordValidity::Rejected) => ValidationStatus::Rejected,
            Err(v) => {
                return Err(crate::query::StateQueryError::Other(format!(
                    "invalid validation_status {v} on StoreRecord op for {hash:?}"
                )))
            }
        };

        // Resolve the record via store-or-scratch (so the entry can come from
        // either source).
        let Some(record) = self
            .retrieve_record_with_scratch(hash, author, scratch)
            .await?
        else {
            return Ok(None);
        };

        // Store deletes + scratch deletes targeting this action hash.
        let store_deletes = self.db().get_delete_actions_for_record(hash).await?;
        let mut deletes: Vec<holochain_zome_types::record::SignedActionHashed> = store_deletes
            .iter()
            .map(holochain_zome_types::dht_v2::to_legacy_signed_action)
            .collect();
        deletes.extend(scratch_deletes_for_record(scratch, hash)?);

        // Store updates + scratch updates targeting this action hash.
        let store_updates = self.db().get_update_actions_for_record(hash).await?;
        let mut updates: Vec<holochain_zome_types::record::SignedActionHashed> = store_updates
            .iter()
            .map(holochain_zome_types::dht_v2::to_legacy_signed_action)
            .collect();
        updates.extend(scratch_updates_for_record(scratch, hash)?);

        Ok(Some(holochain_zome_types::metadata::RecordDetails {
            record,
            validation_status,
            deletes,
            updates,
        }))
    }

    /// Assemble the [`EntryDetails`] for `entry_hash`, overlaying the in-memory
    /// scratch for creates, deletes, updates, and Live/Dead status.
    ///
    /// Returns `None` when the entry is absent from both the store and the scratch.
    ///
    /// The returned fields are assembled as follows:
    /// - **`actions`** (accepted creates): store accepted creates ∪ scratch
    ///   `Create`/`Update` actions whose new-entry side equals `entry_hash`.
    ///   Scratch creates carry no validation status, so they are treated as
    ///   unconditionally accepted.
    /// - **`rejected_actions`**: store only (the scratch holds no validation status).
    /// - **`deletes`**: store deletes ∪ scratch `Delete` actions whose
    ///   `deletes_entry_address == entry_hash`.
    /// - **`updates`**: store updates ∪ scratch `Update` actions whose
    ///   `original_entry_address == entry_hash`.
    /// - **`entry_dht_status`**: Live when there is at least one live create in the
    ///   union of store live creates and scratch live creates (those not targeted by
    ///   a scratch `Delete`).
    ///
    /// Use this on the **requester** path only.
    pub async fn get_entry_details_with_scratch(
        &self,
        entry_hash: &holo_hash::EntryHash,
        author: Option<&holo_hash::AgentPubKey>,
        scratch: &SyncScratch,
    ) -> StateQueryResult<Option<holochain_zome_types::metadata::EntryDetails>> {
        // Resolve the entry from store-or-scratch; return None if absent in both.
        let entry = match self.retrieve_entry(entry_hash, author).await? {
            Some(e) => e,
            None => match scratch_entry(scratch, entry_hash)? {
                Some(e) => e,
                None => return Ok(None),
            },
        };

        let to_legacy = holochain_zome_types::dht_v2::to_legacy_signed_action;

        // Accepted creates: store accepted creates + all scratch creates for this entry.
        let store_accepted = self
            .db()
            .get_entry_creates(entry_hash, author, i64::from(RecordValidity::Accepted))
            .await?;
        let mut actions: Vec<holochain_zome_types::record::SignedActionHashed> =
            store_accepted.iter().map(to_legacy).collect();
        actions.extend(scratch_creates_for_entry(scratch, entry_hash)?);

        // Rejected creates: store only (scratch has no validation status).
        let rejected_actions = self
            .db()
            .get_entry_creates(entry_hash, author, i64::from(RecordValidity::Rejected))
            .await?
            .iter()
            .map(to_legacy)
            .collect();

        // Deletes: store + scratch.
        let store_deletes = self.db().get_delete_actions_for_entry(entry_hash).await?;
        let mut deletes: Vec<holochain_zome_types::record::SignedActionHashed> =
            store_deletes.iter().map(to_legacy).collect();
        deletes.extend(scratch_deletes_for_entry(scratch, entry_hash)?);

        // Updates: store + scratch.
        let store_updates = self.db().get_update_actions_for_entry(entry_hash).await?;
        let mut updates: Vec<holochain_zome_types::record::SignedActionHashed> =
            store_updates.iter().map(to_legacy).collect();
        updates.extend(scratch_updates_for_entry(scratch, entry_hash)?);

        // Live/Dead: Live if there is any undeleted create in the store-or-scratch
        // union. Use get_live_entry_with_scratch as the authoritative liveness check.
        let entry_dht_status = if self
            .get_live_entry_with_scratch(entry_hash, author, scratch)
            .await?
            .is_some()
        {
            holochain_zome_types::metadata::EntryDhtStatus::Live
        } else {
            holochain_zome_types::metadata::EntryDhtStatus::Dead
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

    /// The live links on `base`, overlaid with the in-memory scratch.
    ///
    /// Extends [`get_links`](Self::get_links) with two scratch-layer adjustments:
    ///
    /// - **Scratch `DeleteLink` tombstones**: any store live link whose
    ///   `create_link_hash` is targeted by a scratch `DeleteLink` is removed.
    ///   (`get_links` only excludes *store* delete-link tombstones, so pending
    ///   scratch deletes must be excluded here.)
    /// - **Scratch `CreateLink` additions**: `CreateLink` actions in the scratch
    ///   whose `base_address == base` are included if they are not tombstoned by
    ///   a store `DeleteLink` OR a scratch `DeleteLink`, and they pass the same
    ///   type/tag/author/time filter.
    ///
    /// Use this on the **requester** path only. Authority handlers must never
    /// see scratch data — use [`get_links`](Self::get_links) there.
    pub async fn get_links_with_scratch(
        &self,
        base: &holo_hash::AnyLinkableHash,
        type_query: &holochain_zome_types::prelude::LinkTypeFilter,
        tag: Option<&holochain_zome_types::prelude::LinkTag>,
        filter: &crate::query::link::GetLinksFilter,
        scratch: &SyncScratch,
    ) -> StateQueryResult<Vec<holochain_zome_types::link::Link>> {
        // Start with store live links, already filtered by store delete tombstones.
        let mut store_links = self.get_links(base, type_query, tag, filter).await?;

        // Collect scratch delete-link targets (link_add_address of each scratch
        // DeleteLink) so we can exclude any store link they tombstone.
        let scratch_dl_targets = scratch_delete_link_targets(scratch)?;

        // Exclude store links tombstoned by a scratch DeleteLink.
        store_links.retain(|l| !scratch_dl_targets.contains(&l.create_link_hash));

        // Add scratch CreateLinks for this base that pass all filters.
        let scratch_creates = scratch_create_links_for_base(scratch, base)?;
        for sah in scratch_creates {
            let action = sah.action();
            let holochain_zome_types::action::Action::CreateLink(cl) = action else {
                continue;
            };
            let create_hash = sah.as_hash();

            // Exclude if tombstoned by a store DeleteLink.
            if !self
                .db()
                .get_delete_link_actions(create_hash)
                .await?
                .is_empty()
            {
                continue;
            }
            // Exclude if tombstoned by a scratch DeleteLink.
            if scratch_dl_targets.contains(create_hash) {
                continue;
            }
            // Apply the same type/tag/author/time filter.
            if !type_query.contains(&cl.zome_index, &cl.link_type) {
                continue;
            }
            if let Some(t) = tag {
                if !cl.tag.0.starts_with(&t.0) {
                    continue;
                }
            }
            if let Some(author) = &filter.author {
                if cl.author != *author {
                    continue;
                }
            }
            if let Some(before) = filter.before {
                if cl.timestamp > before {
                    continue;
                }
            }
            if let Some(after) = filter.after {
                if cl.timestamp < after {
                    continue;
                }
            }
            store_links.push(holochain_zome_types::link::Link {
                author: cl.author.clone(),
                base: cl.base_address.clone(),
                target: cl.target_address.clone(),
                timestamp: cl.timestamp,
                zome_index: cl.zome_index,
                link_type: cl.link_type,
                tag: cl.tag.clone(),
                create_link_hash: create_hash.clone(),
            });
        }

        Ok(store_links)
    }

    /// For `base`, every `CreateLink` (live and tombstoned) matching
    /// `type_query`/`tag`, paired with its `DeleteLink`s — overlaid with the
    /// in-memory scratch.
    ///
    /// Extends [`get_link_details`](Self::get_link_details) with two
    /// scratch-layer adjustments:
    ///
    /// - **Scratch `DeleteLink` augmentations**: each store create's delete list
    ///   is extended with scratch `DeleteLink`s targeting that create (i.e.,
    ///   `link_add_address == create_hash`).
    /// - **Scratch `CreateLink` additions**: `CreateLink` actions in the scratch
    ///   whose `base_address == base` are included (ALL — tombstoned or not,
    ///   mirroring `get_link_details` showing every create and its deletes), each
    ///   paired with store deletes + scratch deletes targeting it, filtered by
    ///   type/tag.
    ///
    /// Use this on the **requester** path only.
    pub async fn get_link_details_with_scratch(
        &self,
        base: &holo_hash::AnyLinkableHash,
        type_query: &holochain_zome_types::prelude::LinkTypeFilter,
        tag: Option<&holochain_zome_types::prelude::LinkTag>,
        scratch: &SyncScratch,
    ) -> StateQueryResult<
        Vec<(
            holochain_zome_types::record::SignedActionHashed,
            Vec<holochain_zome_types::record::SignedActionHashed>,
        )>,
    > {
        // Start from store details (creates + their store deletes).
        let mut store_details = self.get_link_details(base, type_query, tag).await?;

        // Collect scratch DeleteLinks indexed by the create they tombstone.
        let scratch_dl_by_create = scratch_delete_links_by_create(scratch)?;

        // Augment each store create's delete list with scratch DeleteLinks
        // targeting that create.
        for (create_sah, deletes) in &mut store_details {
            let create_hash = create_sah.as_hash();
            if let Some(scratch_deletes) = scratch_dl_by_create.get(create_hash) {
                deletes.extend(scratch_deletes.iter().cloned());
            }
        }

        // Add scratch CreateLinks for this base, each paired with their deletes.
        let scratch_creates = scratch_create_links_for_base(scratch, base)?;
        for sah in scratch_creates {
            let action = sah.action();
            let holochain_zome_types::action::Action::CreateLink(cl) = action else {
                continue;
            };
            let create_hash = sah.as_hash();
            // Apply type/tag filter.
            if !type_query.contains(&cl.zome_index, &cl.link_type) {
                continue;
            }
            if let Some(t) = tag {
                if !cl.tag.0.starts_with(&t.0) {
                    continue;
                }
            }
            // Collect deletes: store deletes + scratch deletes targeting this create.
            let mut deletes: Vec<holochain_zome_types::record::SignedActionHashed> = self
                .db()
                .get_delete_link_actions(create_hash)
                .await?
                .iter()
                .map(holochain_zome_types::dht_v2::to_legacy_signed_action)
                .collect();
            if let Some(scratch_deletes) = scratch_dl_by_create.get(create_hash) {
                deletes.extend(scratch_deletes.iter().cloned());
            }
            store_details.push((sah, deletes));
        }

        Ok(store_details)
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

    /// Agent activity for `author`, overlaid with the in-memory scratch.
    ///
    /// Extends [`get_agent_activity`](Self::get_agent_activity) with scratch-authored
    /// actions and warrants:
    ///
    /// - **Scratch activity**: every action in the scratch authored by `author` is
    ///   treated as valid (no rejection can exist for uncommitted actions) and merged
    ///   into the valid set. Scratch activity extends `highest_observed` and shifts
    ///   the chain status accordingly.
    /// - **Scratch warrants**: warrants in the scratch targeting `author` are merged
    ///   into the warrant list when `options.include_warrants` is true.
    ///
    /// The store-only rejected list is passed through unchanged — the scratch holds
    /// no rejected activity.
    ///
    /// Use this on the **requester** path only. Authority handlers must never see
    /// scratch data — use [`get_agent_activity`](Self::get_agent_activity) there.
    pub async fn get_agent_activity_with_scratch(
        &self,
        author: &holo_hash::AgentPubKey,
        filter: &ChainQueryFilter,
        options: &crate::dht_store::GetAgentActivityOptions,
        scratch: &SyncScratch,
    ) -> StateQueryResult<AgentActivityResponse> {
        use holochain_zome_types::dht_v2::to_legacy_signed_action;
        use holochain_zome_types::record::Record;

        let items = self
            .db()
            .get_agent_activity(author.clone(), options.include_full_records)
            .await?;

        // Collect store warrants (gated by option).
        let store_warrants: Vec<holochain_zome_types::prelude::SignedWarrant> =
            if options.include_warrants {
                self.db()
                    .get_warrants_by_warrantee(author.clone())
                    .await?
                    .into_iter()
                    .map(warrant_row_to_signed_warrant)
                    .collect::<StateQueryResult<Vec<_>>>()?
            } else {
                Vec::new()
            };

        // Collect scratch activity + warrants in one lock.
        let (scratch_valid, scratch_warrant_ops) = scratch.apply_and_then(
            |s| -> StateQueryResult<(
                Vec<RegisterAgentActivity>,
                Vec<holochain_types::warrant::WarrantOp>,
            )> {
                let activity = agent_activity_from_scratch(s, author, None, None);
                let warrants = if options.include_warrants {
                    warrants_for_agent_from_scratch(s, author)
                } else {
                    Vec::new()
                };
                Ok((activity, warrants))
            },
        )?;

        // Convert store warrants to WarrantOps and merge with scratch warrants.
        let store_warrant_ops: Vec<holochain_types::warrant::WarrantOp> = store_warrants
            .into_iter()
            .map(holochain_types::warrant::WarrantOp::from)
            .collect();
        let merged_warrant_ops = merge_warrants(vec![store_warrant_ops, scratch_warrant_ops]);
        // Convert back to SignedWarrant for AgentActivityResponse (WarrantOp derefs to it).
        let warrants: Vec<holochain_zome_types::prelude::SignedWarrant> = merged_warrant_ops
            .into_iter()
            .map(|op| (*op).clone())
            .collect();

        if options.include_full_records {
            let mut store_valid_activity = Vec::new();
            let mut rejected = Vec::new();
            for item in items {
                match item.validation_status {
                    RecordValidity::Accepted => store_valid_activity.push(RegisterAgentActivity {
                        action: to_legacy_signed_action(&item.action),
                        cached_entry: None,
                    }),
                    // The rejected list is Records, and keeps the store row's entry.
                    RecordValidity::Rejected => rejected.push(Record::new(
                        to_legacy_signed_action(&item.action),
                        item.entry,
                    )),
                }
            }
            // Merge store and scratch valid activity, dedup by action hash.
            let merged_activity = merge_agent_activity(vec![store_valid_activity, scratch_valid]);
            // Valid records carry no entry here, mirroring the cascade's
            // `cached_entry: None` convention on the requester path (the entry is
            // filled separately by the cascade); rejected records above keep theirs.
            let merged_valid: Vec<Record> = merged_activity
                .into_iter()
                .map(|a| Record::new(a.action, None))
                .collect();
            Ok(build_agent_activity_response(
                author.clone(),
                merged_valid,
                rejected,
                warrants,
                filter,
                options,
            ))
        } else {
            let mut store_valid_activity = Vec::new();
            let mut rejected_hashed = Vec::new();
            for item in items {
                match item.validation_status {
                    RecordValidity::Accepted => store_valid_activity.push(RegisterAgentActivity {
                        action: to_legacy_signed_action(&item.action),
                        cached_entry: None,
                    }),
                    RecordValidity::Rejected => {
                        rejected_hashed.push(to_legacy_signed_action(&item.action).hashed)
                    }
                }
            }
            // Merge store and scratch valid activity, dedup by action hash.
            let merged_activity = merge_agent_activity(vec![store_valid_activity, scratch_valid]);
            // Convert to ActionHashed for build_agent_activity_response.
            let merged_valid: Vec<holochain_zome_types::action::ActionHashed> = merged_activity
                .into_iter()
                .map(|a| a.action.hashed)
                .collect();
            Ok(build_agent_activity_response(
                author.clone(),
                merged_valid,
                rejected_hashed,
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

    /// Store-and-scratch `must_get_agent_activity`: resolve `filter.chain_top`,
    /// scan the bounded `RegisterAgentActivity` range from both the store and the
    /// in-memory scratch, merge, exclude forked actions, apply timestamp/take
    /// filters, and decide completeness.
    ///
    /// The scratch is consulted as a fallback for chain-top and until-hash
    /// resolution (store first, then scratch), and its activity is merged into
    /// the store-scanned range via [`merge_agent_activity`]. Warrants are
    /// attached (from store + scratch) only when the response is `Complete`.
    ///
    /// Use this on the **requester** path only. Authority handlers must never
    /// see scratch data — use [`must_get_agent_activity`](Self::must_get_agent_activity) there.
    pub async fn must_get_agent_activity_with_scratch(
        &self,
        author: &holo_hash::AgentPubKey,
        filter: &ChainFilter,
        scratch: &SyncScratch,
    ) -> StateQueryResult<MustGetAgentActivityResponse> {
        use holochain_zome_types::dht_v2::to_legacy_signed_action;

        // A take of zero is a degenerate filter.
        if filter.get_take() == Some(0) {
            return Err(crate::query::StateQueryError::InvalidInput(
                "ChainFilter take must be greater than 0".to_string(),
            ));
        }

        // Resolve the chain top: store first, scratch fallback.
        let maybe_chain_top = self
            .db()
            .get_action_seq_and_timestamp(author.clone(), filter.chain_top.clone())
            .await?;
        let (chain_top_seq, chain_top_timestamp) = match maybe_chain_top {
            Some(pair) => pair,
            None => {
                // Store miss — try the scratch.
                let from_scratch =
                    scratch.apply_and_then(|s| {
                        Ok::<_, crate::query::StateQueryError>(
                            action_seq_and_timestamp_from_scratch(s, author, &filter.chain_top),
                        )
                    })?;
                match from_scratch {
                    Some(pair) => pair,
                    None => {
                        return Ok(MustGetAgentActivityResponse::ChainTopNotFound(
                            filter.chain_top.clone(),
                        ));
                    }
                }
            }
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

        // Resolve the until_hash lower bound (store first, scratch fallback).
        let mut resolved_until_seq = None;
        if let Some(until_hash) = filter.get_until_hash() {
            // Try the store.
            resolved_until_seq = self
                .db()
                .get_action_seq_and_timestamp(author.clone(), until_hash.clone())
                .await?
                .map(|(seq, _)| seq);
            // Fall back to scratch on a store miss.
            if resolved_until_seq.is_none() {
                resolved_until_seq = scratch.apply_and_then(|s| {
                    Ok::<_, crate::query::StateQueryError>(
                        action_seq_and_timestamp_from_scratch(s, author, until_hash)
                            .map(|(seq, _)| seq),
                    )
                })?;
            }
            if let Some(until_seq) = resolved_until_seq {
                if until_seq > chain_top_seq {
                    return Ok(MustGetAgentActivityResponse::UntilHashAfterChainHead(
                        until_hash.clone(),
                    ));
                }
            }
        }

        // Scan the store bounded range.
        let store_activity: Vec<RegisterAgentActivity> = self
            .db()
            .get_filtered_agent_activity(author.clone(), chain_top_seq, resolved_until_seq)
            .await?
            .into_iter()
            .map(|v2| RegisterAgentActivity {
                action: to_legacy_signed_action(&v2),
                cached_entry: None,
            })
            .collect();

        // Scan the scratch bounded range.
        let scratch_activity: Vec<RegisterAgentActivity> = scratch.apply_and_then(|s| {
            Ok::<_, crate::query::StateQueryError>(agent_activity_from_scratch(
                s,
                author,
                Some(chain_top_seq),
                resolved_until_seq,
            ))
        })?;

        // Merge and deduplicate store + scratch activity.
        let mut activity = merge_agent_activity(vec![store_activity, scratch_activity]);

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
                // Store warrants.
                let store_warrant_ops: Vec<WarrantOp> = self
                    .db()
                    .get_warrants_by_warrantee(author.clone())
                    .await?
                    .into_iter()
                    .map(warrant_row_to_signed_warrant)
                    .collect::<StateQueryResult<Vec<_>>>()?
                    .into_iter()
                    .map(WarrantOp::from)
                    .collect();
                // Scratch warrants.
                let scratch_warrant_ops: Vec<WarrantOp> = scratch.apply_and_then(|s| {
                    Ok::<_, crate::query::StateQueryError>(warrants_for_agent_from_scratch(
                        s, author,
                    ))
                })?;
                let warrants = merge_warrants(vec![store_warrant_ops, scratch_warrant_ops]);
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

    /// Authority-serving create actions for entry `entry_hash` (locally-validated
    /// only), each paired with its (legacy) validation status. Returns legacy types.
    pub async fn get_authority_entry_creates(
        &self,
        entry_hash: &holo_hash::EntryHash,
    ) -> StateQueryResult<
        Vec<(
            holochain_zome_types::record::SignedActionHashed,
            ValidationStatus,
        )>,
    > {
        Ok(self
            .db()
            .get_authority_entry_creates(entry_hash)
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

    /// Authority-serving deletes targeting entry `entry_hash` (locally-validated
    /// only), each paired with its (legacy) validation status.
    pub async fn get_authority_deletes_for_entry(
        &self,
        entry_hash: &holo_hash::EntryHash,
    ) -> StateQueryResult<
        Vec<(
            holochain_zome_types::record::SignedActionHashed,
            ValidationStatus,
        )>,
    > {
        Ok(self
            .db()
            .get_authority_deletes_for_entry(entry_hash)
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

    /// Authority-serving updates targeting entry `entry_hash` (locally-validated
    /// only), each paired with its (legacy) validation status.
    pub async fn get_authority_updates_for_entry(
        &self,
        entry_hash: &holo_hash::EntryHash,
    ) -> StateQueryResult<
        Vec<(
            holochain_zome_types::record::SignedActionHashed,
            ValidationStatus,
        )>,
    > {
        Ok(self
            .db()
            .get_authority_updates_for_entry(entry_hash)
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

    /// Warrants held against `warrantee`'s chain, as legacy signed warrants.
    ///
    /// Used by the get authorities to pair a warrant with any `Rejected`
    /// record they serve, so the receiver can verify the rejection without
    /// being forced into pointless validation work.
    pub async fn get_warrants_by_warrantee(
        &self,
        warrantee: holo_hash::AgentPubKey,
    ) -> StateQueryResult<Vec<SignedWarrant>> {
        self.db()
            .get_warrants_by_warrantee(warrantee)
            .await?
            .into_iter()
            .map(warrant_row_to_signed_warrant)
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

/// Whether `action`'s entry (if any) is declared private.
fn action_entry_is_private(action: &holochain_zome_types::record::SignedActionHashed) -> bool {
    action.action().entry_visibility()
        == Some(&holochain_zome_types::prelude::EntryVisibility::Private)
}

/// Whether the requesting `author` may have `action`'s entry attached to the
/// assembled record. Always true for public entries; for a **private** entry
/// only when the caller is the action's author. A private entry must never be
/// served to a different author — not even when the caller holds a *same-hash*
/// private entry of their own (the entry hash is shared by content, but the
/// privacy is per author). This restores the legacy requester read's
/// entry-visibility hiding that the by-hash `get_entry` lookup alone does not
/// provide.
fn private_entry_visible_to(
    action: &holochain_zome_types::record::SignedActionHashed,
    author: Option<&holo_hash::AgentPubKey>,
) -> bool {
    !action_entry_is_private(action) || author == Some(action.action().author())
}

/// Map the v2 record validity to the legacy validation status served on the wire.
fn record_validity_to_status(v: RecordValidity) -> ValidationStatus {
    match v {
        RecordValidity::Accepted => ValidationStatus::Valid,
        RecordValidity::Rejected => ValidationStatus::Rejected,
    }
}

/// Look up a [`SignedActionHashed`] by `hash` in the scratch space.
///
/// Returns `Ok(None)` when the action is not present. Maps
/// [`SyncScratchError`](crate::scratch::SyncScratchError) into
/// [`StateQueryError`](crate::query::StateQueryError) via the existing `From` impl.
fn scratch_action(
    scratch: &SyncScratch,
    hash: &holo_hash::ActionHash,
) -> StateQueryResult<Option<holochain_zome_types::record::SignedActionHashed>> {
    // Delegate to the `Store` impl on `Scratch` rather than re-scanning, so any
    // future change to how the scratch resolves an action flows through here.
    use crate::query::Store;
    scratch.apply_and_then(|s| s.get_action(hash))
}

/// Look up an [`Entry`] by `hash` in the scratch space.
///
/// Entries in the scratch are authored by this agent and are therefore always
/// visible, regardless of the `author` parameter. (Analogue: the legacy
/// `Scratch::get_public_or_authored_entry` ignores `author` for the same
/// reason.)
fn scratch_entry(
    scratch: &SyncScratch,
    hash: &holo_hash::EntryHash,
) -> StateQueryResult<Option<holochain_types::prelude::Entry>> {
    // Delegate to the `Store` impl on `Scratch` (which likewise returns the
    // entry regardless of author — scratch data is this agent's own).
    use crate::query::Store;
    scratch.apply_and_then(|s| s.get_entry(hash))
}

/// The action hashes tombstoned by a `Delete` in this (locked) scratch — the
/// `deletes_address` of every scratch `Delete`. Operates on a `&Scratch` so it
/// can be reused inside an existing `apply`/`apply_and_then` closure without
/// re-locking.
fn delete_targets_in(
    s: &crate::scratch::Scratch,
) -> std::collections::HashSet<holo_hash::ActionHash> {
    s.actions()
        .filter_map(|sah| match sah.action() {
            holochain_zome_types::action::Action::Delete(d) => Some(d.deletes_address.clone()),
            _ => None,
        })
        .collect()
}

/// The action hashes tombstoned by a pending scratch `Delete`.
fn scratch_delete_targets(
    scratch: &SyncScratch,
) -> StateQueryResult<std::collections::HashSet<holo_hash::ActionHash>> {
    scratch.apply_and_then(|s| Ok::<_, crate::query::StateQueryError>(delete_targets_in(s)))
}

/// Return all `Create`/`Update` actions in the scratch whose `entry_hash()`
/// equals `entry_hash` and that are not targeted by a scratch `Delete`.
///
/// This is the scratch-side analogue of
/// [`get_live_entry_creates`](holochain_data::DbRead::get_live_entry_creates):
/// it collects candidates and filters out any that have a pending delete
/// tombstone in the same scratch.
fn scratch_live_entry_creates(
    scratch: &SyncScratch,
    entry_hash: &holo_hash::EntryHash,
) -> StateQueryResult<Vec<holochain_zome_types::record::SignedActionHashed>> {
    scratch.apply_and_then(|s| {
        // Action hashes of all scratch Deletes, so tombstoned creates are excluded.
        let deleted_addresses = delete_targets_in(s);

        let creates: Vec<holochain_zome_types::record::SignedActionHashed> = s
            .actions()
            .filter(|sah| {
                // Must be a Create or Update referencing this entry hash.
                let references_entry = sah.action().entry_hash() == Some(entry_hash);
                if !references_entry {
                    return false;
                }
                matches!(
                    sah.action(),
                    holochain_zome_types::action::Action::Create(_)
                        | holochain_zome_types::action::Action::Update(_)
                )
            })
            .filter(|sah| {
                // Exclude if a Delete in the scratch targets this action.
                !deleted_addresses.contains(sah.action_address())
            })
            .cloned()
            .collect();

        Ok::<Vec<_>, crate::query::StateQueryError>(creates)
    })
}

/// Return all `Delete` scratch actions whose `deletes_address` equals `hash`
/// (record-level tombstones for `get_record_details`).
fn scratch_deletes_for_record(
    scratch: &SyncScratch,
    hash: &holo_hash::ActionHash,
) -> StateQueryResult<Vec<holochain_zome_types::record::SignedActionHashed>> {
    scratch.apply_and_then(|s| {
        let out = s
            .actions()
            .filter(|sah| match sah.action() {
                holochain_zome_types::action::Action::Delete(d) => &d.deletes_address == hash,
                _ => false,
            })
            .cloned()
            .collect();
        Ok::<Vec<_>, crate::query::StateQueryError>(out)
    })
}

/// Return all `Update` scratch actions whose `original_action_address` equals
/// `hash` (record-level updates for `get_record_details`).
fn scratch_updates_for_record(
    scratch: &SyncScratch,
    hash: &holo_hash::ActionHash,
) -> StateQueryResult<Vec<holochain_zome_types::record::SignedActionHashed>> {
    scratch.apply_and_then(|s| {
        let out = s
            .actions()
            .filter(|sah| match sah.action() {
                holochain_zome_types::action::Action::Update(u) => {
                    &u.original_action_address == hash
                }
                _ => false,
            })
            .cloned()
            .collect();
        Ok::<Vec<_>, crate::query::StateQueryError>(out)
    })
}

/// Return all `Delete` scratch actions whose `deletes_entry_address` equals
/// `entry_hash` (entry-level tombstones for `get_entry_details`).
fn scratch_deletes_for_entry(
    scratch: &SyncScratch,
    entry_hash: &holo_hash::EntryHash,
) -> StateQueryResult<Vec<holochain_zome_types::record::SignedActionHashed>> {
    scratch.apply_and_then(|s| {
        let out = s
            .actions()
            .filter(|sah| match sah.action() {
                holochain_zome_types::action::Action::Delete(d) => {
                    &d.deletes_entry_address == entry_hash
                }
                _ => false,
            })
            .cloned()
            .collect();
        Ok::<Vec<_>, crate::query::StateQueryError>(out)
    })
}

/// Return all `Update` scratch actions whose `original_entry_address` equals
/// `entry_hash` (entry-level updates for `get_entry_details`).
fn scratch_updates_for_entry(
    scratch: &SyncScratch,
    entry_hash: &holo_hash::EntryHash,
) -> StateQueryResult<Vec<holochain_zome_types::record::SignedActionHashed>> {
    scratch.apply_and_then(|s| {
        let out = s
            .actions()
            .filter(|sah| match sah.action() {
                holochain_zome_types::action::Action::Update(u) => {
                    &u.original_entry_address == entry_hash
                }
                _ => false,
            })
            .cloned()
            .collect();
        Ok::<Vec<_>, crate::query::StateQueryError>(out)
    })
}

/// Return all `Create`/`Update` scratch actions whose new-entry side
/// (`entry_hash()`) equals `entry_hash`. This is the accepted-creates analogue
/// for `get_entry_details` — the new entry being introduced, not the one being
/// replaced.
///
/// Unlike [`scratch_live_entry_creates`], this does **not** exclude tombstoned
/// creates: `EntryDetails.actions` lists every create regardless of deletes
/// (liveness is reported separately via `entry_dht_status`).
fn scratch_creates_for_entry(
    scratch: &SyncScratch,
    entry_hash: &holo_hash::EntryHash,
) -> StateQueryResult<Vec<holochain_zome_types::record::SignedActionHashed>> {
    scratch.apply_and_then(|s| {
        let out = s
            .actions()
            .filter(|sah| {
                matches!(
                    sah.action(),
                    holochain_zome_types::action::Action::Create(_)
                        | holochain_zome_types::action::Action::Update(_)
                ) && sah.action().entry_hash() == Some(entry_hash)
            })
            .cloned()
            .collect();
        Ok::<Vec<_>, crate::query::StateQueryError>(out)
    })
}

/// Return all scratch `CreateLink` actions whose `base_address` equals `base`.
///
/// Used by the requester path to collect pending authored links for a base.
fn scratch_create_links_for_base(
    scratch: &SyncScratch,
    base: &holo_hash::AnyLinkableHash,
) -> StateQueryResult<Vec<holochain_zome_types::record::SignedActionHashed>> {
    scratch.apply_and_then(|s| {
        let out = s
            .actions()
            .filter(|sah| match sah.action() {
                holochain_zome_types::action::Action::CreateLink(cl) => &cl.base_address == base,
                _ => false,
            })
            .cloned()
            .collect();
        Ok::<Vec<_>, crate::query::StateQueryError>(out)
    })
}

/// Return the `link_add_address` of every `DeleteLink` in the scratch — the
/// `ActionHash`es of the `CreateLink` actions they tombstone.
///
/// Used by `get_links_with_scratch` to exclude store links tombstoned by a
/// pending scratch `DeleteLink`.
fn scratch_delete_link_targets(
    scratch: &SyncScratch,
) -> StateQueryResult<std::collections::HashSet<holo_hash::ActionHash>> {
    scratch.apply_and_then(|s| {
        let out = s
            .actions()
            .filter_map(|sah| match sah.action() {
                holochain_zome_types::action::Action::DeleteLink(dl) => {
                    Some(dl.link_add_address.clone())
                }
                _ => None,
            })
            .collect();
        Ok::<_, crate::query::StateQueryError>(out)
    })
}

/// Build a map from `CreateLink` action hash → scratch `DeleteLink` actions
/// that tombstone it. Used by `get_link_details_with_scratch` to augment each
/// create's delete list with pending scratch deletes.
fn scratch_delete_links_by_create(
    scratch: &SyncScratch,
) -> StateQueryResult<
    std::collections::HashMap<
        holo_hash::ActionHash,
        Vec<holochain_zome_types::record::SignedActionHashed>,
    >,
> {
    scratch.apply_and_then(|s| {
        let mut map: std::collections::HashMap<
            holo_hash::ActionHash,
            Vec<holochain_zome_types::record::SignedActionHashed>,
        > = std::collections::HashMap::new();
        for sah in s.actions() {
            if let holochain_zome_types::action::Action::DeleteLink(dl) = sah.action() {
                map.entry(dl.link_add_address.clone())
                    .or_default()
                    .push(sah.clone());
            }
        }
        Ok::<_, crate::query::StateQueryError>(map)
    })
}

// ---- scratch helpers for agent activity ----

/// Collect all actions authored by `author` from the scratch, optionally
/// bounded by `chain_top_seq` (inclusive upper bound) and `until_seq`
/// (inclusive lower bound). Used inside a single `apply_and_then` closure so
/// the `SyncScratch` mutex is held for only one lock.
fn agent_activity_from_scratch(
    s: &crate::scratch::Scratch,
    author: &holo_hash::AgentPubKey,
    // `None` = no upper bound (the whole authored chain); `Some(seq)` bounds to
    // `action_seq <= seq` for the `must_get` chain-top window.
    chain_top_seq: Option<u32>,
    until_seq: Option<u32>,
) -> Vec<RegisterAgentActivity> {
    s.actions()
        .filter(|sah| {
            let action = sah.action();
            if action.author() != author {
                return false;
            }
            let seq = action.action_seq();
            if let Some(top) = chain_top_seq {
                if seq > top {
                    return false;
                }
            }
            if let Some(until) = until_seq {
                if seq < until {
                    return false;
                }
            }
            true
        })
        .map(|sah| RegisterAgentActivity {
            action: sah.clone(),
            // Entries are not cached in scratch activity (mirrors cascade TODO).
            cached_entry: None,
        })
        .collect()
}

/// Look up the action sequence and timestamp for `action_hash` from the scratch,
/// filtered to `author`. Returns `None` when the action is absent.
fn action_seq_and_timestamp_from_scratch(
    s: &crate::scratch::Scratch,
    author: &holo_hash::AgentPubKey,
    action_hash: &holo_hash::ActionHash,
) -> Option<(u32, holochain_types::prelude::Timestamp)> {
    s.actions()
        .find(|sah| sah.action().author() == author && &sah.hashed.hash == action_hash)
        .map(|sah| (sah.action().action_seq(), sah.action().timestamp()))
}

/// Collect all scratch warrants whose subject (`InvalidChainOp.action_author` or
/// `ChainFork.chain_author`) matches `agent`.
fn warrants_for_agent_from_scratch(
    s: &crate::scratch::Scratch,
    agent: &holo_hash::AgentPubKey,
) -> Vec<holochain_types::warrant::WarrantOp> {
    use holochain_zome_types::warrant::{ChainIntegrityWarrant, WarrantProof};
    s.warrants()
        .filter(|sw| {
            let WarrantProof::ChainIntegrity(ref w) = sw.proof;
            match w {
                ChainIntegrityWarrant::InvalidChainOp {
                    ref action_author, ..
                } => action_author == agent,
                ChainIntegrityWarrant::ChainFork {
                    ref chain_author, ..
                } => chain_author == agent,
            }
        })
        .map(|sw| holochain_types::warrant::WarrantOp::from(sw.clone()))
        .collect()
}

/// Flatten, sort and deduplicate `RegisterAgentActivity` lists.
///
/// Sort key: `(Reverse(seq), Reverse(hash))` — newest action first, then by
/// hash descending for stability.
fn merge_agent_activity(lists: Vec<Vec<RegisterAgentActivity>>) -> Vec<RegisterAgentActivity> {
    use std::cmp::Reverse;
    let total: usize = lists.iter().map(|l| l.len()).sum();
    let mut merged = Vec::with_capacity(total);
    for list in lists {
        merged.extend(list);
    }
    merged.sort_unstable_by_key(|a| {
        (
            Reverse(a.action.seq()),
            Reverse(a.action.hashed.hash.clone()),
        )
    });
    merged.dedup_by_key(|a| a.action.hashed.hash.clone());
    merged
}

/// Flatten, sort and deduplicate `WarrantOp` lists.
///
/// Sort key: `to_hash()` — stable ordering by op hash.
fn merge_warrants(
    lists: Vec<Vec<holochain_types::warrant::WarrantOp>>,
) -> Vec<holochain_types::warrant::WarrantOp> {
    use holo_hash::HashableContentExtSync;
    let total: usize = lists.iter().map(|l| l.len()).sum();
    let mut merged = Vec::with_capacity(total);
    for list in lists {
        merged.extend(list);
    }
    merged.sort_unstable_by_key(|w| w.to_hash());
    merged.dedup_by_key(|w| w.to_hash());
    merged
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

    /// Regression: a **private** entry must be `Hidden` from a non-author, even
    /// when the entry is retrievable by hash (here it sits in the public `Entry`
    /// table — the same exposure a same-hash private entry of the caller's own
    /// produces). The action itself stays visible.
    #[tokio::test]
    async fn retrieve_record_hides_private_entry_from_non_author() {
        use holochain_types::prelude::{AppEntryBytes, Entry, RecordEntry};

        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();

        let alice = AgentPubKey::from_raw_36(vec![1u8; 36]);
        let bobbo = AgentPubKey::from_raw_36(vec![2u8; 36]);

        let entry = Entry::App(AppEntryBytes(
            holochain_serialized_bytes::SerializedBytes::from(
                holochain_serialized_bytes::UnsafeBytes::from(vec![9u8; 8]),
            ),
        ));
        let entry_hash = EntryHash::with_data_sync(&entry);
        let action = Action::Create(Create {
            author: alice.clone(),
            timestamp: Timestamp::from_micros(1000),
            action_seq: 1,
            prev_action: ActionHash::from_raw_36(vec![3u8; 36]),
            entry_type: EntryType::App(AppEntryDef::new(
                0.into(),
                0.into(),
                EntryVisibility::Private,
            )),
            entry_hash: entry_hash.clone(),
            weight: Default::default(),
        });
        let action_hash = ActionHash::with_data_sync(&action);

        // Recording the StoreRecord op with the entry present lands the entry in
        // the public `Entry` table — the leak surface.
        let chain_op = ChainOp::StoreRecord(
            Signature::from([7u8; 64]),
            action,
            RecordEntry::Present(entry.clone()),
        );
        let op = DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(chain_op)));
        store.record_incoming_ops(vec![op]).await.unwrap();

        // A non-author gets the record, but the private entry is Hidden.
        let record = store
            .as_read()
            .retrieve_record(&action_hash, Some(&bobbo))
            .await
            .unwrap()
            .expect("the action is public, so a record is returned");
        assert_eq!(
            *record.entry(),
            RecordEntry::Hidden,
            "a private entry must be Hidden from a non-author"
        );

        // The author sees their own private entry.
        let record = store
            .as_read()
            .retrieve_record(&action_hash, Some(&alice))
            .await
            .unwrap()
            .expect("author's record");
        assert!(
            matches!(*record.entry(), RecordEntry::Present(_)),
            "the author sees their own private entry"
        );
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

    // ---- get_agent_activity_with_scratch tests ----

    /// Build a scratch `SignedActionHashed` (legacy) for a `Create` by `author`
    /// at the given `action_seq`, linked from `prev`.
    fn make_scratch_create(
        author: &AgentPubKey,
        seq: u32,
        prev: &ActionHash,
        seed: u8,
    ) -> holochain_zome_types::record::SignedActionHashed {
        let action = Action::Create(Create {
            author: author.clone(),
            timestamp: Timestamp::from_micros(seed as i64 * 10_000),
            action_seq: seq,
            prev_action: prev.clone(),
            entry_type: EntryType::App(AppEntryDef::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            )),
            entry_hash: EntryHash::from_raw_36(vec![seed.wrapping_add(200); 36]),
            weight: Default::default(),
        });
        let action_hashed = holochain_zome_types::action::ActionHashed::from_content_sync(action);
        holochain_zome_types::record::SignedActionHashed::with_presigned(
            action_hashed,
            Signature::from([seed; 64]),
        )
    }

    /// Build a scratch `SignedWarrant` whose `InvalidChainOp.action_author` == `warrantee`.
    fn make_scratch_warrant_for(
        warrantee: &AgentPubKey,
        seed: u8,
    ) -> holochain_zome_types::prelude::SignedWarrant {
        use holochain_zome_types::op::ChainOpType;
        use holochain_zome_types::prelude::{
            ChainIntegrityWarrant, SignedWarrant, Warrant, WarrantProof,
        };
        let action_author = warrantee.clone();
        let action_hash = ActionHash::from_raw_36(vec![seed.wrapping_add(150); 36]);
        SignedWarrant::new(
            Warrant::new(
                WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
                    action_author,
                    action: (action_hash, Signature::from([seed; 64])),
                    chain_op_type: ChainOpType::StoreRecord,
                    reason: "scratch warrant".into(),
                }),
                AgentPubKey::from_raw_36(vec![seed.wrapping_add(50); 36]),
                Timestamp::from_micros(seed as i64 * 5_000),
                warrantee.clone(),
            ),
            Signature::from([seed.wrapping_add(1); 64]),
        )
    }

    /// A scratch action for `author` appears in the valid activity list of
    /// `get_agent_activity_with_scratch` and shifts `highest_observed`.
    #[tokio::test]
    async fn get_agent_activity_with_scratch_includes_scratch_action() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let author = AgentPubKey::from_raw_36(vec![110u8; 36]);
        let prev_hash = ActionHash::from_raw_36(vec![0u8; 36]);

        // One integrated action at seq 0.
        integrate_activity(
            &store,
            make_fork_op(&author, &prev_hash, 0, 1),
            AppOutcome::Accepted,
            10,
        )
        .await;

        // Scratch action at seq 1 (linked from some hash — only seq matters here).
        let scratch_prev = ActionHash::from_raw_36(vec![111u8; 36]);
        let scratch_sah = make_scratch_create(&author, 1, &scratch_prev, 111);
        let mut scratch = crate::scratch::Scratch::new();
        scratch.add_action(
            scratch_sah,
            holochain_zome_types::action::ChainTopOrdering::Relaxed,
        );
        let sync_scratch = scratch.into_sync();

        let opts = GetAgentActivityOptions {
            include_valid_activity: true,
            include_rejected_activity: false,
            include_warrants: false,
            include_full_records: false,
        };
        let resp = store
            .as_read()
            .get_agent_activity_with_scratch(
                &author,
                &ChainQueryFilter::new(),
                &opts,
                &sync_scratch,
            )
            .await
            .unwrap();

        // Both seq 0 (store) and seq 1 (scratch) should be valid.
        match &resp.valid_activity {
            ChainItems::Hashes(h) => {
                assert_eq!(h.len(), 2, "expected both store and scratch action");
                let seqs: Vec<u32> = h.iter().map(|(seq, _)| *seq).collect();
                assert!(seqs.contains(&0), "seq 0 (store) should be present");
                assert!(seqs.contains(&1), "seq 1 (scratch) should be present");
            }
            other => panic!("expected Hashes, got {other:?}"),
        }
        // highest_observed should reflect seq 1 from the scratch.
        let ho = resp
            .highest_observed
            .expect("highest_observed should be set");
        assert_eq!(
            ho.action_seq, 1,
            "highest_observed should include scratch action"
        );
    }

    /// A scratch action for `author` appears in the full-records list.
    #[tokio::test]
    async fn get_agent_activity_with_scratch_full_records_includes_scratch_action() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let author = AgentPubKey::from_raw_36(vec![112u8; 36]);
        let prev_hash = ActionHash::from_raw_36(vec![0u8; 36]);

        integrate_activity(
            &store,
            make_fork_op(&author, &prev_hash, 0, 1),
            AppOutcome::Accepted,
            10,
        )
        .await;

        let scratch_prev = ActionHash::from_raw_36(vec![113u8; 36]);
        let scratch_sah = make_scratch_create(&author, 1, &scratch_prev, 113);
        let mut scratch = crate::scratch::Scratch::new();
        scratch.add_action(
            scratch_sah,
            holochain_zome_types::action::ChainTopOrdering::Relaxed,
        );
        let sync_scratch = scratch.into_sync();

        let opts = GetAgentActivityOptions {
            include_valid_activity: true,
            include_rejected_activity: false,
            include_warrants: false,
            include_full_records: true,
        };
        let resp = store
            .as_read()
            .get_agent_activity_with_scratch(
                &author,
                &ChainQueryFilter::new(),
                &opts,
                &sync_scratch,
            )
            .await
            .unwrap();

        match &resp.valid_activity {
            ChainItems::Full(records) => {
                assert_eq!(records.len(), 2, "expected store record + scratch record");
            }
            other => panic!("expected Full, got {other:?}"),
        }
    }

    /// A scratch warrant targeting `author` appears in the response when
    /// `include_warrants = true`.
    #[tokio::test]
    async fn get_agent_activity_with_scratch_includes_scratch_warrant() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let author = AgentPubKey::from_raw_36(vec![114u8; 36]);
        let prev_hash = ActionHash::from_raw_36(vec![0u8; 36]);

        integrate_activity(
            &store,
            make_fork_op(&author, &prev_hash, 0, 1),
            AppOutcome::Accepted,
            10,
        )
        .await;

        // Scratch contains one warrant for `author`.
        let signed_warrant = make_scratch_warrant_for(&author, 114);
        let mut scratch = crate::scratch::Scratch::new();
        scratch.add_warrant(signed_warrant);
        let sync_scratch = scratch.into_sync();

        let opts_with = GetAgentActivityOptions {
            include_valid_activity: true,
            include_rejected_activity: false,
            include_warrants: true,
            include_full_records: false,
        };
        let resp = store
            .as_read()
            .get_agent_activity_with_scratch(
                &author,
                &ChainQueryFilter::new(),
                &opts_with,
                &sync_scratch,
            )
            .await
            .unwrap();
        assert_eq!(resp.warrants.len(), 1, "scratch warrant should be present");

        // When include_warrants = false the scratch warrant should be absent.
        let opts_without = GetAgentActivityOptions {
            include_valid_activity: true,
            include_rejected_activity: false,
            include_warrants: false,
            include_full_records: false,
        };
        let resp = store
            .as_read()
            .get_agent_activity_with_scratch(
                &author,
                &ChainQueryFilter::new(),
                &opts_without,
                &sync_scratch,
            )
            .await
            .unwrap();
        assert!(
            resp.warrants.is_empty(),
            "warrants should be empty when not requested"
        );
    }

    /// Store-only `get_agent_activity` ignores a populated scratch.
    #[tokio::test]
    async fn get_agent_activity_store_only_ignores_scratch() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let author = AgentPubKey::from_raw_36(vec![115u8; 36]);
        let prev_hash = ActionHash::from_raw_36(vec![0u8; 36]);

        integrate_activity(
            &store,
            make_fork_op(&author, &prev_hash, 0, 1),
            AppOutcome::Accepted,
            10,
        )
        .await;

        // Scratch action at seq 1 — must NOT appear in the store-only read.
        let scratch_prev = ActionHash::from_raw_36(vec![116u8; 36]);
        let scratch_sah = make_scratch_create(&author, 1, &scratch_prev, 116);
        let mut scratch = crate::scratch::Scratch::new();
        scratch.add_action(
            scratch_sah,
            holochain_zome_types::action::ChainTopOrdering::Relaxed,
        );
        // (scratch is intentionally unused in the store-only call)
        let _ = scratch;

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

        match &resp.valid_activity {
            ChainItems::Hashes(h) => {
                assert_eq!(
                    h.len(),
                    1,
                    "store-only read must not include scratch actions"
                );
                assert_eq!(h[0].0, 0, "only seq 0 from the store");
            }
            other => panic!("expected Hashes, got {other:?}"),
        }
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

    // ---- must_get_agent_activity_with_scratch tests ----

    /// Scratch-only chain top resolves correctly: the store has no such action
    /// but the scratch does, so the response is not `ChainTopNotFound`.
    #[tokio::test]
    async fn must_get_agent_activity_with_scratch_chain_top_from_scratch() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let author = AgentPubKey::from_raw_36(vec![150u8; 36]);

        // Build a 1-action chain and integrate seq 0 into the store.
        let (ops, hashes) = make_activity_chain(&author, 1);
        integrate_activity(&store, ops[0].clone(), AppOutcome::Accepted, 10).await;

        // The scratch action at seq 1 is linked to hashes[0] (the store action).
        // Its content-derived hash is what we use as chain_top.
        let scratch_sah = make_scratch_create(&author, 1, &hashes[0], 150);
        let scratch_chain_top = scratch_sah.as_hash().clone();
        let mut scratch = crate::scratch::Scratch::new();
        scratch.add_action(
            scratch_sah,
            holochain_zome_types::action::ChainTopOrdering::Relaxed,
        );
        let sync_scratch = scratch.into_sync();

        // filter.chain_top == scratch_chain_top (scratch-only action hash).
        let filter = ChainFilter::new(scratch_chain_top.clone());
        let resp = store
            .as_read()
            .must_get_agent_activity_with_scratch(&author, &filter, &sync_scratch)
            .await
            .unwrap();

        // The scratch resolved the chain top, so the result must not be
        // ChainTopNotFound.
        assert!(
            !matches!(resp, MustGetAgentActivityResponse::ChainTopNotFound(_)),
            "chain top should have been resolved from the scratch"
        );
    }

    /// A scratch-authored action within the bounded range appears in the
    /// merged activity of the response.
    #[tokio::test]
    async fn must_get_agent_activity_with_scratch_includes_scratch_activity() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let author = AgentPubKey::from_raw_36(vec![151u8; 36]);

        // Build a 3-action chain (seqs 0..=2) in the store.
        let (ops, hashes) = make_activity_chain(&author, 3);
        for (i, op) in ops.into_iter().enumerate() {
            integrate_activity(&store, op, AppOutcome::Accepted, 10 + i as i64).await;
        }

        // Scratch action at seq 3, linked to the store action at seq 2 (hashes[2]).
        // The scratch action's content-derived hash becomes the chain_top for the filter.
        let scratch_sah = make_scratch_create(&author, 3, &hashes[2], 151);
        let scratch_hash = scratch_sah.as_hash().clone();
        let mut scratch = crate::scratch::Scratch::new();
        scratch.add_action(
            scratch_sah,
            holochain_zome_types::action::ChainTopOrdering::Relaxed,
        );
        let sync_scratch = scratch.into_sync();

        // Use the scratch action's hash as chain_top so chain_top_seq == 3.
        let filter = ChainFilter::new(scratch_hash.clone());
        let resp = store
            .as_read()
            .must_get_agent_activity_with_scratch(&author, &filter, &sync_scratch)
            .await
            .unwrap();

        match resp {
            MustGetAgentActivityResponse::Activity { activity, .. } => {
                let seqs: Vec<u32> = activity.iter().map(|a| a.action.seq()).collect();
                assert!(
                    seqs.contains(&3),
                    "scratch action at seq 3 should be in merged activity; got {seqs:?}"
                );
                assert!(
                    seqs.contains(&2),
                    "store action at seq 2 should be in merged activity; got {seqs:?}"
                );
            }
            other => panic!("expected Activity, got {other:?}"),
        }
    }

    /// Store-only `must_get_agent_activity` ignores a populated scratch.
    #[tokio::test]
    async fn must_get_agent_activity_store_only_ignores_scratch() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let author = AgentPubKey::from_raw_36(vec![152u8; 36]);
        let (ops, hashes) = make_activity_chain(&author, 3);
        for (i, op) in ops.into_iter().enumerate() {
            integrate_activity(&store, op, AppOutcome::Accepted, 10 + i as i64).await;
        }

        // Scratch action at seq 3 — must NOT appear in the store-only read.
        let scratch_sah = make_scratch_create(&author, 3, &hashes[2], 152);
        let mut scratch = crate::scratch::Scratch::new();
        scratch.add_action(
            scratch_sah,
            holochain_zome_types::action::ChainTopOrdering::Relaxed,
        );
        // scratch intentionally not passed to the store-only call
        let _ = scratch;

        let filter = ChainFilter::new(hashes[2].clone());
        let resp = store
            .as_read()
            .must_get_agent_activity(&author, &filter)
            .await
            .unwrap();

        match resp {
            MustGetAgentActivityResponse::Activity { activity, .. } => {
                assert_eq!(
                    activity.len(),
                    3,
                    "store-only read must not include scratch actions"
                );
                let seqs: Vec<u32> = activity.iter().map(|a| a.action.seq()).collect();
                assert!(
                    !seqs.contains(&3),
                    "scratch action at seq 3 must not appear"
                );
            }
            other => panic!("expected Activity, got {other:?}"),
        }
    }

    /// take == 0 returns `InvalidInput` with a scratch present.
    #[tokio::test]
    async fn must_get_agent_activity_with_scratch_take_zero_errors() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let author = AgentPubKey::from_raw_36(vec![153u8; 36]);
        let (ops, hashes) = make_activity_chain(&author, 1);
        for (i, op) in ops.into_iter().enumerate() {
            integrate_activity(&store, op, AppOutcome::Accepted, 10 + i as i64).await;
        }
        let filter = ChainFilter::take(hashes[0].clone(), 0);
        let empty_scratch = crate::scratch::Scratch::new().into_sync();
        let result = store
            .as_read()
            .must_get_agent_activity_with_scratch(&author, &filter, &empty_scratch)
            .await;
        assert!(result.is_err(), "take == 0 must return an error");
    }

    /// chain_top absent from both store and scratch → `ChainTopNotFound`.
    #[tokio::test]
    async fn must_get_agent_activity_with_scratch_chain_top_absent_in_both() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let author = AgentPubKey::from_raw_36(vec![154u8; 36]);
        let unknown = ActionHash::from_raw_36(vec![99u8; 36]);
        let empty_scratch = crate::scratch::Scratch::new().into_sync();
        let resp = store
            .as_read()
            .must_get_agent_activity_with_scratch(
                &author,
                &ChainFilter::new(unknown.clone()),
                &empty_scratch,
            )
            .await
            .unwrap();
        assert!(
            matches!(resp, MustGetAgentActivityResponse::ChainTopNotFound(h) if h == unknown),
            "chain top absent from both sources must yield ChainTopNotFound"
        );
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

    // ---- scratch overlay helpers ----

    /// Build a `SignedActionHashed` (legacy) for a `Create` that references
    /// `entry_hash`. Useful for scratch overlay tests.
    fn make_signed_action_for_entry(
        seed: u8,
        entry_hash: EntryHash,
    ) -> holochain_zome_types::record::SignedActionHashed {
        let action = Action::Create(Create {
            author: AgentPubKey::from_raw_36(vec![seed; 36]),
            timestamp: Timestamp::from_micros(seed as i64 * 1000),
            action_seq: 1,
            prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(200); 36]),
            entry_type: EntryType::App(AppEntryDef::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            )),
            entry_hash,
            weight: Default::default(),
        });
        let action_hashed = holochain_zome_types::action::ActionHashed::from_content_sync(action);
        holochain_zome_types::record::SignedActionHashed::with_presigned(
            action_hashed,
            Signature::from([seed; 64]),
        )
    }

    /// Build a `SignedActionHashed` (legacy) for a `Create` with no entry
    /// (simulates an action that is in the scratch but whose entry is missing).
    fn make_signed_action_no_entry(seed: u8) -> holochain_zome_types::record::SignedActionHashed {
        // Use a Dna action — it carries no entry hash.
        use holochain_zome_types::action::Dna;
        let action = Action::Dna(Dna {
            author: AgentPubKey::from_raw_36(vec![seed; 36]),
            timestamp: Timestamp::from_micros(seed as i64 * 1000),
            hash: holo_hash::DnaHash::from_raw_36(vec![seed; 36]),
        });
        let action_hashed = holochain_zome_types::action::ActionHashed::from_content_sync(action);
        holochain_zome_types::record::SignedActionHashed::with_presigned(
            action_hashed,
            Signature::from([seed; 64]),
        )
    }

    /// Build a scratch containing a single action.
    fn scratch_with_action(
        sah: holochain_zome_types::record::SignedActionHashed,
    ) -> crate::scratch::SyncScratch {
        let mut scratch = crate::scratch::Scratch::new();
        scratch.add_action(sah, holochain_zome_types::action::ChainTopOrdering::Relaxed);
        scratch.into_sync()
    }

    /// Build a scratch containing an action + its entry.
    fn scratch_with_action_and_entry(
        sah: holochain_zome_types::record::SignedActionHashed,
        entry: holochain_types::prelude::Entry,
        entry_hash: EntryHash,
    ) -> crate::scratch::SyncScratch {
        let mut scratch = crate::scratch::Scratch::new();
        scratch.add_action(sah, holochain_zome_types::action::ChainTopOrdering::Relaxed);
        let entry_hashed =
            holochain_types::prelude::EntryHashed::with_pre_hashed(entry, entry_hash);
        scratch.add_entry(
            entry_hashed,
            holochain_zome_types::action::ChainTopOrdering::Relaxed,
        );
        scratch.into_sync()
    }

    // ---- scratch overlay tests ----

    /// (a) action present only in store → `retrieve_action_with_scratch` returns it.
    #[tokio::test]
    async fn retrieve_action_with_scratch_finds_store_action() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let op = make_chain_op(90);
        let action = match op.as_content() {
            DhtOp::ChainOp(c) => c.action().clone(),
            _ => unreachable!(),
        };
        let action_hash = holo_hash::ActionHash::with_data_sync(&action);
        store.record_incoming_ops(vec![op]).await.unwrap();

        // Empty scratch — action lives only in the store.
        let empty_scratch = crate::scratch::Scratch::new().into_sync();
        let result = store
            .as_read()
            .retrieve_action_with_scratch(&action_hash, &empty_scratch)
            .await
            .unwrap();
        assert!(result.is_some(), "should find store-only action");
        assert_eq!(result.unwrap().as_hash(), &action_hash);
    }

    /// (b) action present only in scratch → `retrieve_action_with_scratch` returns it.
    #[tokio::test]
    async fn retrieve_action_with_scratch_finds_scratch_action() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let entry_hash = EntryHash::from_raw_36(vec![91u8.wrapping_add(100); 36]);
        let sah = make_signed_action_for_entry(91, entry_hash);
        let action_hash = sah.as_hash().clone();
        let scratch = scratch_with_action(sah);

        // The action was never written to the store.
        let result = store
            .as_read()
            .retrieve_action_with_scratch(&action_hash, &scratch)
            .await
            .unwrap();
        assert!(result.is_some(), "should find scratch-only action");
        assert_eq!(result.unwrap().as_hash(), &action_hash);
    }

    /// (d) store-only `retrieve_action` ignores a scratch-only action.
    #[tokio::test]
    async fn retrieve_action_ignores_scratch() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let entry_hash = EntryHash::from_raw_36(vec![92u8.wrapping_add(100); 36]);
        let sah = make_signed_action_for_entry(92, entry_hash);
        let action_hash = sah.as_hash().clone();

        // The action exists only in the scratch — not in the store.
        let result = store.as_read().retrieve_action(&action_hash).await.unwrap();
        assert!(
            result.is_none(),
            "store-only retrieve_action must not see scratch data"
        );
    }

    /// entry present only in store → `retrieve_entry_with_scratch` returns it.
    #[tokio::test]
    async fn retrieve_entry_with_scratch_finds_store_entry() {
        use holochain_types::dht_op::{ChainOp, DhtOp};
        use holochain_types::prelude::{AppEntryBytes, Entry};

        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();

        // Build a StoreEntry op that carries a public entry.
        let seed = 93u8;
        let author = AgentPubKey::from_raw_36(vec![seed; 36]);
        let entry_bytes = holochain_serialized_bytes::SerializedBytes::from(
            holochain_serialized_bytes::UnsafeBytes::from(vec![seed; 8]),
        );
        let entry = Entry::App(AppEntryBytes(entry_bytes));
        let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);

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
        let chain_op = ChainOp::StoreEntry(
            Signature::from([seed; 64]),
            action.try_into().unwrap(),
            entry.clone(),
        );
        let op = holochain_types::dht_op::DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(
            chain_op,
        )));
        store.record_incoming_ops(vec![op]).await.unwrap();

        let empty_scratch = crate::scratch::Scratch::new().into_sync();
        let result = store
            .as_read()
            .retrieve_entry_with_scratch(&entry_hash, None, &empty_scratch)
            .await
            .unwrap();
        assert!(result.is_some(), "should find store-only entry");
    }

    /// entry present only in scratch → `retrieve_entry_with_scratch` returns it.
    #[tokio::test]
    async fn retrieve_entry_with_scratch_finds_scratch_entry() {
        use holochain_types::prelude::{AppEntryBytes, Entry};

        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();

        let seed = 94u8;
        let entry_bytes = holochain_serialized_bytes::SerializedBytes::from(
            holochain_serialized_bytes::UnsafeBytes::from(vec![seed; 8]),
        );
        let entry = Entry::App(AppEntryBytes(entry_bytes));
        let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
        let sah = make_signed_action_for_entry(seed, entry_hash.clone());
        let scratch = scratch_with_action_and_entry(sah, entry.clone(), entry_hash.clone());

        let result = store
            .as_read()
            .retrieve_entry_with_scratch(&entry_hash, None, &scratch)
            .await
            .unwrap();
        assert!(result.is_some(), "should find scratch-only entry");
    }

    /// `retrieve_record_with_scratch` with action in scratch + entry in scratch.
    #[tokio::test]
    async fn retrieve_record_with_scratch_action_and_entry_in_scratch() {
        use holochain_types::prelude::{AppEntryBytes, Entry};

        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();

        let seed = 95u8;
        let entry_bytes = holochain_serialized_bytes::SerializedBytes::from(
            holochain_serialized_bytes::UnsafeBytes::from(vec![seed; 8]),
        );
        let entry = Entry::App(AppEntryBytes(entry_bytes));
        let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
        let sah = make_signed_action_for_entry(seed, entry_hash.clone());
        let action_hash = sah.as_hash().clone();
        let scratch = scratch_with_action_and_entry(sah, entry, entry_hash);

        let result = store
            .as_read()
            .retrieve_record_with_scratch(&action_hash, None, &scratch)
            .await
            .unwrap();
        assert!(result.is_some(), "record with action+entry both in scratch");
    }

    /// (c) `retrieve_record_with_scratch` with action in scratch but its
    /// referenced entry nowhere → `None`.
    #[tokio::test]
    async fn retrieve_record_with_scratch_missing_entry_returns_none() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();

        let seed = 96u8;
        let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
        // Action references an entry, but the entry is not in scratch or store.
        let sah = make_signed_action_for_entry(seed, entry_hash);
        let action_hash = sah.as_hash().clone();
        let scratch = scratch_with_action(sah);

        let result = store
            .as_read()
            .retrieve_record_with_scratch(&action_hash, None, &scratch)
            .await
            .unwrap();
        assert!(result.is_none(), "missing entry must yield None");
    }

    /// `retrieve_record_with_scratch` with an action that carries no entry
    /// (e.g. `Dna`) → returns the record.
    #[tokio::test]
    async fn retrieve_record_with_scratch_no_entry_action() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();

        let sah = make_signed_action_no_entry(97);
        let action_hash = sah.as_hash().clone();
        let scratch = scratch_with_action(sah);

        let result = store
            .as_read()
            .retrieve_record_with_scratch(&action_hash, None, &scratch)
            .await
            .unwrap();
        assert!(
            result.is_some(),
            "action with no entry should still produce a record"
        );
    }

    /// Action resolved from the store, but its referenced entry only in the
    /// scratch → record assembled from both sources. Exercises the cross-path
    /// where `retrieve_record` alone returns `None`.
    #[tokio::test]
    async fn retrieve_record_with_scratch_store_action_scratch_entry() {
        use holochain_types::prelude::{AppEntryBytes, Entry, EntryHashed};

        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();

        // `make_chain_op` records a RegisterAgentActivity op: the Create action
        // (referencing entry `seed+100`) lands in the store, but the entry does not.
        let seed = 94u8;
        let op = make_chain_op(seed);
        let action = match op.as_content() {
            DhtOp::ChainOp(c) => c.action().clone(),
            _ => unreachable!(),
        };
        let action_hash = holo_hash::ActionHash::with_data_sync(&action);
        let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
        store.record_incoming_ops(vec![op]).await.unwrap();

        // Store-only read returns None: the action references an entry the store
        // does not hold.
        let store_only = store
            .as_read()
            .retrieve_record(&action_hash, None)
            .await
            .unwrap();
        assert!(store_only.is_none(), "entry is absent from the store");

        // The entry lives only in the scratch.
        let entry = Entry::App(AppEntryBytes(
            holochain_serialized_bytes::SerializedBytes::from(
                holochain_serialized_bytes::UnsafeBytes::from(vec![seed; 8]),
            ),
        ));
        let mut scratch = crate::scratch::Scratch::new();
        scratch.add_entry(
            EntryHashed::with_pre_hashed(entry, entry_hash),
            holochain_zome_types::action::ChainTopOrdering::Relaxed,
        );
        let scratch = scratch.into_sync();

        // The overlay resolves the action from the store and the entry from the scratch.
        let result = store
            .as_read()
            .retrieve_record_with_scratch(&action_hash, None, &scratch)
            .await
            .unwrap();
        let record = result.expect("record assembled from store action + scratch entry");
        assert_eq!(record.action_address(), &action_hash);
        assert!(
            record.entry().as_option().is_some(),
            "entry should be supplied by the scratch"
        );
    }

    // ---- get_live_{record,entry}_with_scratch helpers ----

    /// Build a public [`Entry::App`] with payload `vec![seed; 8]`.
    fn make_entry(seed: u8) -> holochain_types::prelude::Entry {
        use holochain_types::prelude::{AppEntryBytes, Entry};
        let bytes = holochain_serialized_bytes::SerializedBytes::from(
            holochain_serialized_bytes::UnsafeBytes::from(vec![seed; 8]),
        );
        Entry::App(AppEntryBytes(bytes))
    }

    /// Build and integrate a `StoreEntry` op so that `entry_hash` has a live
    /// store create. Returns the action hash of the `Create` action.
    async fn integrate_store_entry_op(
        store: &crate::dht_store::DhtStore<DbWrite<Dht>>,
        seed: u8,
        author: &AgentPubKey,
        entry_hash: EntryHash,
        entry: holochain_types::prelude::Entry,
    ) -> ActionHash {
        use crate::dht_store::{AppOutcome, SysOutcome};
        use holochain_types::action::NewEntryAction;
        use holochain_types::dht_op::{ChainOp, DhtOp};

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
        let action_hash = holo_hash::ActionHash::with_data_sync(&action);
        let new_entry_action: NewEntryAction = action.try_into().unwrap();
        let chain_op =
            ChainOp::StoreEntry(Signature::from([seed; 64]), new_entry_action, entry.clone());
        let op = holochain_types::dht_op::DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(
            chain_op,
        )));
        let op_hash = op.as_hash().clone();
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
            .integrate_ready_ops(Timestamp::from_micros(1000))
            .await
            .unwrap();
        action_hash
    }

    /// Build a scratch `Delete` action targeting `deletes_address` /
    /// `deletes_entry_address`, wrapped in a `SyncScratch`.
    fn scratch_with_delete(
        seed: u8,
        deletes_address: ActionHash,
        deletes_entry_address: EntryHash,
    ) -> crate::scratch::SyncScratch {
        use holochain_zome_types::action::Delete;
        let delete = Action::Delete(Delete {
            author: AgentPubKey::from_raw_36(vec![seed; 36]),
            timestamp: Timestamp::from_micros(seed as i64 * 2000),
            action_seq: 2,
            prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(150); 36]),
            deletes_address,
            deletes_entry_address,
            weight: Default::default(),
        });
        let action_hashed = holochain_zome_types::action::ActionHashed::from_content_sync(delete);
        let sah = holochain_zome_types::record::SignedActionHashed::with_presigned(
            action_hashed,
            Signature::from([seed; 64]),
        );
        let mut scratch = crate::scratch::Scratch::new();
        scratch.add_action(sah, holochain_zome_types::action::ChainTopOrdering::Relaxed);
        scratch.into_sync()
    }

    // ---- get_live_record_with_scratch tests ----

    /// (a) Store create + a scratch `Delete` targeting it →
    /// `get_live_record_with_scratch` returns `None`, but
    /// `get_live_record` still returns the record (it ignores the scratch).
    #[tokio::test]
    async fn get_live_record_with_scratch_scratch_delete_tombstones_store_record() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();

        let seed = 110u8;
        let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
        let author = AgentPubKey::from_raw_36(vec![seed; 36]);
        let entry = make_entry(seed);

        // First insert a RegisterAgentActivity op so the action is in the store.
        let act_op = make_chain_op(seed);
        let action = match act_op.as_content() {
            DhtOp::ChainOp(c) => c.action().clone(),
            _ => unreachable!(),
        };
        let action_hash = holo_hash::ActionHash::with_data_sync(&action);
        store.record_incoming_ops(vec![act_op]).await.unwrap();

        // Integrate a StoreEntry op so the record is fully live in the store.
        integrate_store_entry_op(&store, seed, &author, entry_hash.clone(), entry).await;

        // Store-only live-record sees it.
        let store_result = store
            .as_read()
            .get_live_record(&action_hash, None)
            .await
            .unwrap();
        assert!(
            store_result.is_some(),
            "store-only get_live_record should return the record"
        );

        // A scratch Delete targeting the action hash.
        let scratch = scratch_with_delete(seed, action_hash.clone(), entry_hash);

        // With-scratch path returns None due to the scratch tombstone.
        let result = store
            .as_read()
            .get_live_record_with_scratch(&action_hash, None, &scratch)
            .await
            .unwrap();
        assert!(
            result.is_none(),
            "scratch delete should tombstone the record for get_live_record_with_scratch"
        );
    }

    /// (b) Scratch-only create → `get_live_record_with_scratch` and
    /// `get_live_entry_with_scratch` both return the live record.
    #[tokio::test]
    async fn get_live_with_scratch_scratch_only_create_is_live() {
        use holochain_types::prelude::{AppEntryBytes, Entry};

        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();

        let seed = 111u8;
        let entry_bytes = holochain_serialized_bytes::SerializedBytes::from(
            holochain_serialized_bytes::UnsafeBytes::from(vec![seed; 8]),
        );
        let entry = Entry::App(AppEntryBytes(entry_bytes));
        let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
        let sah = make_signed_action_for_entry(seed, entry_hash.clone());
        let action_hash = sah.as_hash().clone();
        let scratch = scratch_with_action_and_entry(sah, entry, entry_hash.clone());

        // get_live_record_with_scratch should find the scratch-only create.
        let record_result = store
            .as_read()
            .get_live_record_with_scratch(&action_hash, None, &scratch)
            .await
            .unwrap();
        assert!(
            record_result.is_some(),
            "scratch-only create should be live for get_live_record_with_scratch"
        );

        // get_live_entry_with_scratch should also find it.
        let entry_result = store
            .as_read()
            .get_live_entry_with_scratch(&entry_hash, None, &scratch)
            .await
            .unwrap();
        assert!(
            entry_result.is_some(),
            "scratch-only create should be live for get_live_entry_with_scratch"
        );
    }

    /// (c) Author-preference: both a store create and a scratch create exist for
    /// the same entry. Querying with the scratch-create author picks the scratch
    /// action; querying with a third author falls back to the store create first.
    #[tokio::test]
    async fn get_live_entry_with_scratch_author_preference() {
        use holochain_types::prelude::{AppEntryBytes, Entry};

        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();

        // Shared entry hash for both creates.
        let entry_hash = EntryHash::from_raw_36(vec![150u8; 36]);
        let entry_bytes = holochain_serialized_bytes::SerializedBytes::from(
            holochain_serialized_bytes::UnsafeBytes::from(vec![1u8; 8]),
        );
        let entry = Entry::App(AppEntryBytes(entry_bytes));

        // Store create by store_author (seed=112).
        let store_author = AgentPubKey::from_raw_36(vec![112u8; 36]);
        let store_action_hash = integrate_store_entry_op(
            &store,
            112u8,
            &store_author,
            entry_hash.clone(),
            entry.clone(),
        )
        .await;

        // Scratch create by scratch_author (seed=113).
        let seed = 113u8;
        let scratch_sah = make_signed_action_for_entry(seed, entry_hash.clone());
        let scratch_action_hash = scratch_sah.as_hash().clone();
        let scratch = scratch_with_action_and_entry(scratch_sah, entry.clone(), entry_hash.clone());

        // Querying with scratch_author → authored pick returns the scratch action.
        let scratch_author = AgentPubKey::from_raw_36(vec![seed; 36]);
        let result_scratch_author = store
            .as_read()
            .get_live_entry_with_scratch(&entry_hash, Some(&scratch_author), &scratch)
            .await
            .unwrap()
            .expect("should find a live record for scratch_author");
        assert_eq!(
            result_scratch_author.action_address(),
            &scratch_action_hash,
            "scratch_author should pick the scratch create"
        );

        // Querying with store_author → authored pick returns the store action.
        let result_store_author = store
            .as_read()
            .get_live_entry_with_scratch(&entry_hash, Some(&store_author), &scratch)
            .await
            .unwrap()
            .expect("should find a live record for store_author");
        assert_eq!(
            result_store_author.action_address(),
            &store_action_hash,
            "store_author should pick the store create"
        );

        // Querying with an unmatched author → falls back to the first store
        // create (store before scratch).
        let other_author = AgentPubKey::from_raw_36(vec![200u8; 36]);
        let result_other = store
            .as_read()
            .get_live_entry_with_scratch(&entry_hash, Some(&other_author), &scratch)
            .await
            .unwrap()
            .expect("should find a live record for other_author");
        assert_eq!(
            result_other.action_address(),
            &store_action_hash,
            "unmatched author should fall back to first store create"
        );
    }

    /// A scratch `Delete` targeting a *store* entry create tombstones it, so
    /// `get_live_entry_with_scratch` returns `None` even though the store-only
    /// `get_live_entry` still returns the record.
    #[tokio::test]
    async fn get_live_entry_with_scratch_scratch_delete_tombstones_store_create() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();

        let seed = 118u8;
        let author = AgentPubKey::from_raw_36(vec![seed; 36]);
        let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
        let entry = make_entry(seed);

        // Integrate a StoreEntry op so the create is live in the store.
        let store_action_hash =
            integrate_store_entry_op(&store, seed, &author, entry_hash.clone(), entry).await;

        // Store-only read sees the live entry.
        let store_only = store
            .as_read()
            .get_live_entry(&entry_hash, None)
            .await
            .unwrap();
        assert!(
            store_only.is_some(),
            "store-only get_live_entry should return the create"
        );

        // A scratch Delete targeting the store create's action hash.
        let scratch = scratch_with_delete(seed, store_action_hash, entry_hash.clone());

        // The overlay must exclude the tombstoned store create → no live creates.
        let result = store
            .as_read()
            .get_live_entry_with_scratch(&entry_hash, None, &scratch)
            .await
            .unwrap();
        assert!(
            result.is_none(),
            "scratch delete of the store create should tombstone get_live_entry_with_scratch"
        );
    }

    /// (d) Store-only `get_live_record` and `get_live_entry` ignore a populated
    /// scratch (the requester-only invariant).
    #[tokio::test]
    async fn get_live_store_only_ignores_scratch() {
        use holochain_types::prelude::{AppEntryBytes, Entry};

        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();

        let seed = 114u8;
        let entry_bytes = holochain_serialized_bytes::SerializedBytes::from(
            holochain_serialized_bytes::UnsafeBytes::from(vec![seed; 8]),
        );
        let entry = Entry::App(AppEntryBytes(entry_bytes));
        let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
        let sah = make_signed_action_for_entry(seed, entry_hash.clone());
        let action_hash = sah.as_hash().clone();

        // The action + entry live only in the scratch.
        // Store-only reads should return None.
        let result_record = store
            .as_read()
            .get_live_record(&action_hash, None)
            .await
            .unwrap();
        assert!(
            result_record.is_none(),
            "get_live_record must not see scratch-only data"
        );

        let result_entry = store
            .as_read()
            .get_live_entry(&entry_hash, None)
            .await
            .unwrap();
        assert!(
            result_entry.is_none(),
            "get_live_entry must not see scratch-only data"
        );

        // Confirm the scratch data is there (sanity check) but is irrelevant
        // to store-only queries.
        let scratch = scratch_with_action_and_entry(sah, entry, entry_hash);
        let _ = scratch; // scratch is intentionally not passed to the store-only methods
    }

    // ---- get_record_details_with_scratch / get_entry_details_with_scratch tests ----

    /// Build and integrate a `StoreRecord` op so that `action_hash` has a live
    /// store record. Returns the action hash. The action has no entry (uses `Dna`).
    async fn integrate_store_record_op(
        store: &crate::dht_store::DhtStore<DbWrite<Dht>>,
        seed: u8,
        author: &AgentPubKey,
        entry_hash: EntryHash,
        entry: holochain_types::prelude::Entry,
    ) -> ActionHash {
        use crate::dht_store::{AppOutcome, SysOutcome};
        use holochain_types::dht_op::{ChainOp, DhtOp};
        use holochain_types::prelude::RecordEntry;

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
        let action_hash = holo_hash::ActionHash::with_data_sync(&action);
        let chain_op = ChainOp::StoreRecord(
            Signature::from([seed; 64]),
            action,
            RecordEntry::Present(entry),
        );
        let op = holochain_types::dht_op::DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(
            chain_op,
        )));
        let op_hash = op.as_hash().clone();
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
            .integrate_ready_ops(Timestamp::from_micros(1000))
            .await
            .unwrap();
        action_hash
    }

    /// Scratch `Update` and scratch `Delete` targeting an integrated store record
    /// appear in the returned `updates`/`deletes` lists.
    #[tokio::test]
    async fn get_record_details_with_scratch_shows_scratch_updates_and_deletes() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();

        let seed = 120u8;
        let author = AgentPubKey::from_raw_36(vec![seed; 36]);
        let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
        let entry = make_entry(seed);

        // Integrate a StoreRecord so the record exists in the store.
        let action_hash =
            integrate_store_record_op(&store, seed, &author, entry_hash.clone(), entry).await;

        // Also integrate a StoreEntry op so the entry is live (for the record retrieval).
        let entry2 = make_entry(seed);
        integrate_store_entry_op(
            &store,
            seed.wrapping_add(1),
            &author,
            entry_hash.clone(),
            entry2,
        )
        .await;

        // Build a scratch with one Delete and one Update targeting the store record.
        let new_entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(50); 36]);
        let mut scratch = crate::scratch::Scratch::new();

        // Delete targeting the store record's action hash.
        use holochain_zome_types::action::Delete;
        let delete = Action::Delete(Delete {
            author: AgentPubKey::from_raw_36(vec![seed.wrapping_add(10); 36]),
            timestamp: Timestamp::from_micros(seed as i64 * 3000),
            action_seq: 3,
            prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(160); 36]),
            deletes_address: action_hash.clone(),
            deletes_entry_address: entry_hash.clone(),
            weight: Default::default(),
        });
        let delete_hashed = holochain_zome_types::action::ActionHashed::from_content_sync(delete);
        let delete_sah = holochain_zome_types::record::SignedActionHashed::with_presigned(
            delete_hashed,
            Signature::from([seed.wrapping_add(10); 64]),
        );
        scratch.add_action(
            delete_sah,
            holochain_zome_types::action::ChainTopOrdering::Relaxed,
        );

        // Update targeting the store record's action hash.
        use holochain_zome_types::action::Update;
        let update = Action::Update(Update {
            author: AgentPubKey::from_raw_36(vec![seed.wrapping_add(20); 36]),
            timestamp: Timestamp::from_micros(seed as i64 * 4000),
            action_seq: 4,
            prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(170); 36]),
            original_action_address: action_hash.clone(),
            original_entry_address: entry_hash.clone(),
            entry_type: EntryType::App(AppEntryDef::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            )),
            entry_hash: new_entry_hash.clone(),
            weight: Default::default(),
        });
        let update_hashed = holochain_zome_types::action::ActionHashed::from_content_sync(update);
        let update_sah = holochain_zome_types::record::SignedActionHashed::with_presigned(
            update_hashed,
            Signature::from([seed.wrapping_add(20); 64]),
        );
        scratch.add_action(
            update_sah,
            holochain_zome_types::action::ChainTopOrdering::Relaxed,
        );

        let scratch = scratch.into_sync();

        let details = store
            .as_read()
            .get_record_details_with_scratch(&action_hash, None, &scratch)
            .await
            .unwrap()
            .expect("should find details for integrated record");

        assert_eq!(
            details.deletes.len(),
            1,
            "scratch Delete should appear in deletes"
        );
        assert_eq!(
            details.updates.len(),
            1,
            "scratch Update should appear in updates"
        );
        assert_eq!(
            details.validation_status,
            holochain_zome_types::validate::ValidationStatus::Valid
        );
    }

    /// A scratch-only action (no integrated `StoreRecord` op) returns `None`.
    /// Documents the store-gate contract.
    #[tokio::test]
    async fn get_record_details_with_scratch_returns_none_for_scratch_only_action() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();

        let seed = 121u8;
        let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
        // Action is only in the scratch — never written to the store.
        let sah = make_signed_action_for_entry(seed, entry_hash.clone());
        let action_hash = sah.as_hash().clone();
        let scratch = scratch_with_action(sah);

        let result = store
            .as_read()
            .get_record_details_with_scratch(&action_hash, None, &scratch)
            .await
            .unwrap();
        assert!(
            result.is_none(),
            "scratch-only action must return None (no StoreRecord op)"
        );
    }

    /// Store-only `get_record_details` ignores a populated scratch.
    #[tokio::test]
    async fn get_record_details_ignores_scratch() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();

        let seed = 122u8;
        let author = AgentPubKey::from_raw_36(vec![seed; 36]);
        let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
        let entry = make_entry(seed);

        // Integrate a StoreRecord and a StoreEntry.
        let action_hash =
            integrate_store_record_op(&store, seed, &author, entry_hash.clone(), entry.clone())
                .await;
        integrate_store_entry_op(
            &store,
            seed.wrapping_add(1),
            &author,
            entry_hash.clone(),
            entry,
        )
        .await;

        // Build a scratch with a Delete targeting the record (intentionally not
        // passed to the store-only read — it must be invisible to it).
        let _scratch = scratch_with_delete(seed, action_hash.clone(), entry_hash.clone());

        // Store-only path ignores the scratch delete.
        let details = store
            .as_read()
            .get_record_details(&action_hash, None)
            .await
            .unwrap()
            .expect("store-only get_record_details should find the record");
        assert!(
            details.deletes.is_empty(),
            "store-only get_record_details must not see scratch deletes"
        );
    }

    /// A scratch `Create` for an otherwise-Dead entry flips `entry_dht_status`
    /// to `Live`; scratch deletes and updates appear in the respective lists.
    #[tokio::test]
    async fn get_entry_details_with_scratch_scratch_create_flips_dead_to_live() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();

        let seed = 125u8;
        let author = AgentPubKey::from_raw_36(vec![seed; 36]);
        let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
        let entry = make_entry(seed);

        // Integrate a StoreEntry op that is then deleted (by a store delete) so
        // the entry is Dead in the store.
        let store_action_hash =
            integrate_store_entry_op(&store, seed, &author, entry_hash.clone(), entry.clone())
                .await;

        // Integrate a RegisterDeletedEntryAction so the store-create is deleted.
        {
            use crate::dht_store::{AppOutcome, SysOutcome};
            use holochain_types::dht_op::{ChainOp, DhtOp};
            use holochain_zome_types::action::Delete;
            let delete_action = Action::Delete(Delete {
                author: author.clone(),
                timestamp: Timestamp::from_micros(seed as i64 * 5000),
                action_seq: 3,
                prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(180); 36]),
                deletes_address: store_action_hash.clone(),
                deletes_entry_address: entry_hash.clone(),
                weight: Default::default(),
            });
            let chain_op = ChainOp::RegisterDeletedEntryAction(
                Signature::from([seed.wrapping_add(5); 64]),
                match delete_action {
                    Action::Delete(d) => d,
                    _ => unreachable!(),
                },
            );
            let op = holochain_types::dht_op::DhtOpHashed::from_content_sync(DhtOp::ChainOp(
                Box::new(chain_op),
            ));
            let op_hash = op.as_hash().clone();
            store.record_incoming_ops(vec![op]).await.unwrap();
            store
                .record_chain_op_sys_validation_outcomes(vec![(
                    op_hash.clone(),
                    SysOutcome::Accepted,
                )])
                .await
                .unwrap();
            store
                .record_app_validation_outcomes(vec![(op_hash.clone(), AppOutcome::Accepted)])
                .await
                .unwrap();
            store
                .integrate_ready_ops(Timestamp::from_micros(2000))
                .await
                .unwrap();
        }

        // Confirm the entry is Dead in the store.
        let store_details = store
            .as_read()
            .get_entry_details(&entry_hash, None)
            .await
            .unwrap()
            .expect("entry exists in store");
        assert_eq!(
            store_details.entry_dht_status,
            holochain_zome_types::metadata::EntryDhtStatus::Dead,
            "entry should be Dead in store after store-delete"
        );

        // A scratch Create for the same entry_hash flips the status to Live.
        let scratch_sah = make_signed_action_for_entry(seed.wrapping_add(30), entry_hash.clone());
        let scratch = scratch_with_action_and_entry(scratch_sah, entry.clone(), entry_hash.clone());

        let details = store
            .as_read()
            .get_entry_details_with_scratch(&entry_hash, None, &scratch)
            .await
            .unwrap()
            .expect("entry should be found via scratch");

        assert_eq!(
            details.entry_dht_status,
            holochain_zome_types::metadata::EntryDhtStatus::Live,
            "scratch Create should flip Dead→Live"
        );
        // The scratch create appears in actions.
        assert!(
            !details.actions.is_empty(),
            "scratch Create should appear in actions"
        );
    }

    /// Scratch deletes and updates appear in `get_entry_details_with_scratch`
    /// alongside the store-only lists.
    #[tokio::test]
    async fn get_entry_details_with_scratch_shows_scratch_deletes_and_updates() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();

        let seed = 126u8;
        let author = AgentPubKey::from_raw_36(vec![seed; 36]);
        let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
        let entry = make_entry(seed);

        // Integrate a live StoreEntry op.
        let store_action_hash =
            integrate_store_entry_op(&store, seed, &author, entry_hash.clone(), entry.clone())
                .await;

        let new_entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(60); 36]);

        // Scratch with a Delete (deletes_entry_address == entry_hash) and an
        // Update (original_entry_address == entry_hash).
        let mut scratch = crate::scratch::Scratch::new();

        use holochain_zome_types::action::Delete;
        let delete = Action::Delete(Delete {
            author: AgentPubKey::from_raw_36(vec![seed.wrapping_add(10); 36]),
            timestamp: Timestamp::from_micros(seed as i64 * 3000),
            action_seq: 3,
            prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(160); 36]),
            deletes_address: store_action_hash.clone(),
            deletes_entry_address: entry_hash.clone(),
            weight: Default::default(),
        });
        scratch.add_action(
            holochain_zome_types::record::SignedActionHashed::with_presigned(
                holochain_zome_types::action::ActionHashed::from_content_sync(delete),
                Signature::from([seed.wrapping_add(10); 64]),
            ),
            holochain_zome_types::action::ChainTopOrdering::Relaxed,
        );

        use holochain_zome_types::action::Update;
        let update = Action::Update(Update {
            author: AgentPubKey::from_raw_36(vec![seed.wrapping_add(20); 36]),
            timestamp: Timestamp::from_micros(seed as i64 * 4000),
            action_seq: 4,
            prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(170); 36]),
            original_action_address: store_action_hash.clone(),
            original_entry_address: entry_hash.clone(),
            entry_type: EntryType::App(AppEntryDef::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            )),
            entry_hash: new_entry_hash.clone(),
            weight: Default::default(),
        });
        scratch.add_action(
            holochain_zome_types::record::SignedActionHashed::with_presigned(
                holochain_zome_types::action::ActionHashed::from_content_sync(update),
                Signature::from([seed.wrapping_add(20); 64]),
            ),
            holochain_zome_types::action::ChainTopOrdering::Relaxed,
        );

        let scratch = scratch.into_sync();

        let details = store
            .as_read()
            .get_entry_details_with_scratch(&entry_hash, None, &scratch)
            .await
            .unwrap()
            .expect("entry should be found");

        assert_eq!(
            details.deletes.len(),
            1,
            "scratch Delete should appear in deletes"
        );
        assert_eq!(
            details.updates.len(),
            1,
            "scratch Update should appear in updates"
        );
        // The scratch Delete tombstones the only store create for this entry
        // (the scratch Update creates a *different* entry), so the entry flips
        // Live → Dead under the overlay.
        assert_eq!(
            details.entry_dht_status,
            holochain_zome_types::metadata::EntryDhtStatus::Dead,
            "scratch delete of the only create should make the entry Dead"
        );
    }

    /// Store-only `get_entry_details` ignores a populated scratch.
    #[tokio::test]
    async fn get_entry_details_ignores_scratch() {
        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();

        let seed = 127u8;
        let author = AgentPubKey::from_raw_36(vec![seed; 36]);
        let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
        let entry = make_entry(seed);

        // Integrate a live StoreEntry op.
        let store_action_hash =
            integrate_store_entry_op(&store, seed, &author, entry_hash.clone(), entry.clone())
                .await;

        // Build a scratch with a Delete targeting the store create (intentionally
        // not passed to the store-only read — it must be invisible to it).
        let _scratch = scratch_with_delete(seed, store_action_hash.clone(), entry_hash.clone());

        // Store-only path ignores the scratch delete.
        let details = store
            .as_read()
            .get_entry_details(&entry_hash, None)
            .await
            .unwrap()
            .expect("store-only get_entry_details should find the entry");
        assert!(
            details.deletes.is_empty(),
            "store-only get_entry_details must not see scratch deletes"
        );
        assert_eq!(
            details.entry_dht_status,
            holochain_zome_types::metadata::EntryDhtStatus::Live,
            "store-only get_entry_details must not see scratch deletes for status"
        );
    }

    // ---- get_links_with_scratch / get_link_details_with_scratch helpers ----

    /// Build and integrate a `RegisterAddLink` op into the store.
    /// Returns `(action_hash, base_address)` of the created link.
    async fn integrate_link_op_for_base(
        store: &crate::dht_store::DhtStore<DbWrite<Dht>>,
        base: &holo_hash::AnyLinkableHash,
        zome_index: u8,
        link_type: u8,
        tag_bytes: Vec<u8>,
        seed: u8,
        when: i64,
    ) -> holo_hash::ActionHash {
        use crate::dht_store::{AppOutcome, SysOutcome};
        use holochain_types::dht_op::{ChainOp, DhtOp};
        use holochain_zome_types::action::CreateLink;
        use holochain_zome_types::link::LinkTag;

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
            zome_index: zome_index.into(),
            link_type: link_type.into(),
            tag: LinkTag(tag_bytes),
            weight: Default::default(),
        });
        let create_link = match action {
            Action::CreateLink(cl) => cl,
            _ => unreachable!(),
        };
        let op = DhtOpHashed::from_content_sync(DhtOp::ChainOp(Box::new(
            ChainOp::RegisterAddLink(Signature::from([seed; 64]), create_link),
        )));
        let op_hash = op.as_hash().clone();
        let action_hash = match op.as_content() {
            DhtOp::ChainOp(c) => holo_hash::ActionHash::with_data_sync(&c.action()),
            _ => unreachable!(),
        };
        store.record_incoming_ops(vec![op]).await.unwrap();
        store
            .record_chain_op_sys_validation_outcomes(vec![(op_hash.clone(), SysOutcome::Accepted)])
            .await
            .unwrap();
        store
            .record_app_validation_outcomes(vec![(op_hash, AppOutcome::Accepted)])
            .await
            .unwrap();
        store
            .integrate_ready_ops(Timestamp::from_micros(when))
            .await
            .unwrap();
        action_hash
    }

    /// Build a scratch `CreateLink` action for `base`.
    fn make_scratch_create_link(
        base: &holo_hash::AnyLinkableHash,
        zome_index: u8,
        link_type: u8,
        tag_bytes: Vec<u8>,
        seed: u8,
    ) -> holochain_zome_types::record::SignedActionHashed {
        use holochain_zome_types::action::CreateLink;
        use holochain_zome_types::link::LinkTag;

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
            zome_index: zome_index.into(),
            link_type: link_type.into(),
            tag: LinkTag(tag_bytes),
            weight: Default::default(),
        });
        let action_hashed = holochain_zome_types::action::ActionHashed::from_content_sync(action);
        holochain_zome_types::record::SignedActionHashed::with_presigned(
            action_hashed,
            Signature::from([seed; 64]),
        )
    }

    /// Build a scratch `DeleteLink` action tombstoning `create_link_hash`.
    fn make_scratch_delete_link(
        base: &holo_hash::AnyLinkableHash,
        create_link_hash: holo_hash::ActionHash,
        seed: u8,
    ) -> holochain_zome_types::record::SignedActionHashed {
        use holochain_zome_types::action::DeleteLink;

        let action = Action::DeleteLink(DeleteLink {
            author: AgentPubKey::from_raw_36(vec![seed; 36]),
            timestamp: Timestamp::from_micros(seed as i64 * 1000 + 500),
            action_seq: 3,
            prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(90); 36]),
            base_address: base.clone(),
            link_add_address: create_link_hash,
        });
        let action_hashed = holochain_zome_types::action::ActionHashed::from_content_sync(action);
        holochain_zome_types::record::SignedActionHashed::with_presigned(
            action_hashed,
            Signature::from([seed; 64]),
        )
    }

    // ---- get_links_with_scratch tests ----

    /// A scratch `CreateLink` for `base` appears in `get_links_with_scratch`
    /// and is correctly filtered by type, tag, author, and time.
    #[tokio::test]
    async fn get_links_with_scratch_scratch_create_link_appears_and_is_filtered() {
        use crate::query::link::GetLinksFilter;
        use holochain_zome_types::prelude::LinkTypeFilter;

        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let base = holo_hash::AnyLinkableHash::from_raw_36_and_type(
            vec![130u8; 36],
            holo_hash::hash_type::AnyLinkable::Entry,
        );

        // Build a scratch CreateLink: zome 0, type 0, tag [1,2,3], seed 131.
        let scratch_sah = make_scratch_create_link(&base, 0, 0, vec![1, 2, 3], 131);
        let scratch_create_hash = scratch_sah.as_hash().clone();
        let mut scratch_inner = crate::scratch::Scratch::new();
        scratch_inner.add_action(
            scratch_sah,
            holochain_zome_types::action::ChainTopOrdering::Relaxed,
        );
        let scratch = scratch_inner.into_sync();

        let filter = GetLinksFilter {
            after: None,
            before: None,
            author: None,
        };

        // The link appears with a matching type+tag query.
        let links = store
            .as_read()
            .get_links_with_scratch(
                &base,
                &LinkTypeFilter::Dependencies(vec![0.into()]),
                None,
                &filter,
                &scratch,
            )
            .await
            .unwrap();
        assert_eq!(links.len(), 1, "scratch CreateLink should appear");
        assert_eq!(links[0].create_link_hash, scratch_create_hash);

        // Filtered out by a different zome index.
        let links_no_type = store
            .as_read()
            .get_links_with_scratch(
                &base,
                &LinkTypeFilter::Dependencies(vec![1.into()]),
                None,
                &filter,
                &scratch,
            )
            .await
            .unwrap();
        assert!(
            links_no_type.is_empty(),
            "scratch CreateLink must be excluded by type filter"
        );

        // Filtered out by a non-matching tag prefix.
        let bad_tag = holochain_zome_types::link::LinkTag(vec![9, 9, 9]);
        let links_no_tag = store
            .as_read()
            .get_links_with_scratch(
                &base,
                &LinkTypeFilter::Dependencies(vec![0.into()]),
                Some(&bad_tag),
                &filter,
                &scratch,
            )
            .await
            .unwrap();
        assert!(
            links_no_tag.is_empty(),
            "scratch CreateLink must be excluded by tag filter"
        );

        // Filtered out by author.
        let other_author = AgentPubKey::from_raw_36(vec![200u8; 36]);
        let filter_author = GetLinksFilter {
            after: None,
            before: None,
            author: Some(other_author),
        };
        let links_no_author = store
            .as_read()
            .get_links_with_scratch(
                &base,
                &LinkTypeFilter::Dependencies(vec![0.into()]),
                None,
                &filter_author,
                &scratch,
            )
            .await
            .unwrap();
        assert!(
            links_no_author.is_empty(),
            "scratch CreateLink must be excluded by author filter"
        );

        // Filtered out by `before` (link timestamp is 131*1000; before=130*1000 excludes it).
        let filter_before = GetLinksFilter {
            after: None,
            before: Some(Timestamp::from_micros(130 * 1000)),
            author: None,
        };
        let links_before = store
            .as_read()
            .get_links_with_scratch(
                &base,
                &LinkTypeFilter::Dependencies(vec![0.into()]),
                None,
                &filter_before,
                &scratch,
            )
            .await
            .unwrap();
        assert!(
            links_before.is_empty(),
            "scratch CreateLink must be excluded by before filter"
        );

        // Filtered out by `after` (link timestamp is 131*1000; after=132*1000 excludes it).
        let filter_after = GetLinksFilter {
            after: Some(Timestamp::from_micros(132 * 1000)),
            before: None,
            author: None,
        };
        let links_after = store
            .as_read()
            .get_links_with_scratch(
                &base,
                &LinkTypeFilter::Dependencies(vec![0.into()]),
                None,
                &filter_after,
                &scratch,
            )
            .await
            .unwrap();
        assert!(
            links_after.is_empty(),
            "scratch CreateLink must be excluded by after filter"
        );
    }

    /// A scratch `DeleteLink` targeting a *store* `CreateLink` removes that
    /// link from `get_links_with_scratch`, but both the create and the delete
    /// appear in `get_link_details_with_scratch`.
    #[tokio::test]
    async fn scratch_delete_link_tombstones_store_create_in_get_links_but_not_details() {
        use crate::query::link::GetLinksFilter;
        use holochain_zome_types::prelude::LinkTypeFilter;

        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let base = holo_hash::AnyLinkableHash::from_raw_36_and_type(
            vec![140u8; 36],
            holo_hash::hash_type::AnyLinkable::Entry,
        );

        // Integrate a store CreateLink.
        let store_create_hash =
            integrate_link_op_for_base(&store, &base, 0, 0, vec![1, 2, 3], 141, 100).await;

        // Confirm it appears in store-only get_links.
        let filter = GetLinksFilter {
            after: None,
            before: None,
            author: None,
        };
        let store_links = store
            .as_read()
            .get_links(
                &base,
                &LinkTypeFilter::Dependencies(vec![0.into()]),
                None,
                &filter,
            )
            .await
            .unwrap();
        assert_eq!(store_links.len(), 1, "store CreateLink should be live");

        // Add a scratch DeleteLink targeting the store create.
        let dl_sah = make_scratch_delete_link(&base, store_create_hash.clone(), 142);
        let mut scratch_inner = crate::scratch::Scratch::new();
        scratch_inner.add_action(
            dl_sah,
            holochain_zome_types::action::ChainTopOrdering::Relaxed,
        );
        let scratch = scratch_inner.into_sync();

        // get_links_with_scratch must exclude the tombstoned store link.
        let links = store
            .as_read()
            .get_links_with_scratch(
                &base,
                &LinkTypeFilter::Dependencies(vec![0.into()]),
                None,
                &filter,
                &scratch,
            )
            .await
            .unwrap();
        assert!(
            links.is_empty(),
            "scratch DeleteLink must tombstone the store CreateLink in get_links_with_scratch"
        );

        // get_link_details_with_scratch must show the create AND the scratch delete.
        let details = store
            .as_read()
            .get_link_details_with_scratch(
                &base,
                &LinkTypeFilter::Dependencies(vec![0.into()]),
                None,
                &scratch,
            )
            .await
            .unwrap();
        assert_eq!(
            details.len(),
            1,
            "store CreateLink should appear in details"
        );
        let (create_sah, deletes) = &details[0];
        assert_eq!(
            create_sah.as_hash(),
            &store_create_hash,
            "details create should be the store CreateLink"
        );
        assert_eq!(
            deletes.len(),
            1,
            "scratch DeleteLink must appear in the details delete list"
        );
    }

    /// Store-only `get_links` and `get_link_details` ignore a populated scratch
    /// (requester-only invariant).
    #[tokio::test]
    async fn get_links_store_only_ignores_scratch() {
        use crate::query::link::GetLinksFilter;
        use holochain_zome_types::prelude::LinkTypeFilter;

        let store = crate::dht_store::DhtStore::new_test(dht_id())
            .await
            .unwrap();
        let base = holo_hash::AnyLinkableHash::from_raw_36_and_type(
            vec![150u8; 36],
            holo_hash::hash_type::AnyLinkable::Entry,
        );

        // Integrate a store CreateLink.
        let store_create_hash =
            integrate_link_op_for_base(&store, &base, 0, 0, vec![1, 2, 3], 151, 100).await;

        // Build a scratch with: one new CreateLink + one DeleteLink tombstoning the store create.
        let scratch_cl_sah = make_scratch_create_link(&base, 0, 0, vec![4, 5, 6], 152);
        let scratch_dl_sah = make_scratch_delete_link(&base, store_create_hash.clone(), 153);
        let mut scratch_inner = crate::scratch::Scratch::new();
        scratch_inner.add_action(
            scratch_cl_sah,
            holochain_zome_types::action::ChainTopOrdering::Relaxed,
        );
        scratch_inner.add_action(
            scratch_dl_sah,
            holochain_zome_types::action::ChainTopOrdering::Relaxed,
        );
        // Scratch is built but NOT passed to the store-only methods.
        let _scratch = scratch_inner.into_sync();

        // Store-only get_links: should see only the store link (no scratch create, no exclusion).
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
        assert_eq!(
            links.len(),
            1,
            "store-only get_links must not see scratch CreateLink"
        );
        assert_eq!(
            links[0].create_link_hash, store_create_hash,
            "store-only get_links must return the store link unaffected by scratch delete"
        );

        // Store-only get_link_details: should see only the store create, with no deletes.
        let details = store
            .as_read()
            .get_link_details(&base, &LinkTypeFilter::Dependencies(vec![0.into()]), None)
            .await
            .unwrap();
        assert_eq!(
            details.len(),
            1,
            "store-only get_link_details must not see scratch CreateLink"
        );
        assert_eq!(
            details[0].1.len(),
            0,
            "store-only get_link_details must not see scratch DeleteLink"
        );
    }
}
