use crate::chain_lock::ChainLock;
use crate::prelude::*;
use crate::scratch::ScratchError;
use crate::scratch::SyncScratchError;
use async_recursion::async_recursion;
pub use error::*;
// The authoring pipeline is a scoped LEGACY ISLAND: actions are built,
// signed, and staged in the scratch as legacy `Action`/`SignedActionHashed`/
// `Record`, matching what `scratch.rs` and the legacy SQL query machinery in
// `query.rs` expect. These explicit imports shadow the v2 re-exports pulled
// in via `crate::prelude::*` so the rest of this module keeps resolving
// `Action`/`Record`/`SignedActionHashed` to their legacy shape. A handful of
// functions read through to v2-native `DhtStore` methods; those convert at
// the point of use instead of relying on the module-wide shadow.
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::DhtOpHash;
use holo_hash::DnaHash;
use holo_hash::EntryHash;
use holo_hash::HasHash;
use holochain_data::kind::Dht;
use holochain_data::{DbRead, DbWrite};
use holochain_keystore::MetaLairClient;
use holochain_state_types::SourceChainDump;
use holochain_types::prelude::LegacySignedAction;
use holochain_zome_types::dependencies::holochain_integrity_types::action::Action;
use holochain_zome_types::dependencies::holochain_integrity_types::record::Record;
use holochain_zome_types::dependencies::holochain_integrity_types::record::SignedActionHashed;
use kitsune2_api::DhtArc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;

mod error;

#[derive(Clone)]
pub struct SourceChain<Db = DbWrite<Dht>> {
    scratch: SyncScratch,
    pub(crate) dht_store: DhtStore<Db>,
    keystore: MetaLairClient,
    author: Arc<AgentPubKey>,
    cell_id: Arc<CellId>,
    head_info: Option<HeadInfo>,
    public_only: bool,
    zomes_initialized: Arc<AtomicBool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadInfo {
    pub action: ActionHash,
    pub seq: u32,
    pub timestamp: Timestamp,
}

impl HeadInfo {
    pub fn into_tuple(self) -> (ActionHash, u32, Timestamp) {
        (self.action, self.seq, self.timestamp)
    }
}

/// A source chain with read only access to the underlying database.
pub type SourceChainRead = SourceChain<DbRead<Dht>>;

// TODO: document that many functions here are only reading from the scratch,
//       not the entire source chain!
/// Writable functions for a source chain with write access.
impl SourceChain<DbWrite<Dht>> {
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    pub async fn unlock_chain(&self) -> SourceChainResult<()> {
        // The chain lock lives in the DhtStore.
        self.dht_store
            .release_chain_lock(self.author.as_ref())
            .await?;
        Ok(())
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    pub async fn accept_countersigning_preflight_request(
        &self,
        preflight_request: PreflightRequest,
        agent_index: u8,
    ) -> SourceChainResult<CounterSigningAgentState> {
        let hashed_preflight_request =
            blake2b_256(&holochain_serialized_bytes::encode(&preflight_request)?);

        // This all needs to be ensured in a non-panicky way BEFORE calling into the source chain here.
        let author = self.author.clone();
        assert_eq!(
            *author,
            preflight_request.signing_agents[agent_index as usize].0
        );

        // Check for a chain lock.
        // Note that the lock may not be valid anymore, but we must respect it here anyway.
        // `get_chain_lock` returns any lock row, including an expired one, so an
        // expired lock still rejects acceptance.
        if self
            .dht_store
            .as_read()
            .get_chain_lock(author.as_ref().clone())
            .await?
            .is_some()
        {
            return Err(SourceChainError::ChainLocked);
        }

        let HeadInfo {
            action: persisted_head,
            seq: persisted_seq,
            ..
        } = self
            .dht_store
            .as_read()
            .chain_head_for_author(author.as_ref())
            .await?
            .ok_or(SourceChainError::ChainEmpty)?;
        let countersigning_agent_state =
            CounterSigningAgentState::new(agent_index, persisted_head, persisted_seq);

        // Take out the lock. We verified above that no lock exists, so this must
        // succeed; the bool guards against a concurrent writer slipping a lock in
        // between the check and here, in which case we reject as `ChainLocked`
        // rather than extending or stealing the lock. The head read above and
        // this lock acquisition are separate operations, not one transaction: a
        // concurrent flush can move the head in between, but a stale captured
        // head is rejected by the as-at check when the session commits, so the
        // session fails cleanly rather than forking the chain.
        let acquired = self
            .dht_store
            .acquire_chain_lock(
                author.as_ref(),
                &hashed_preflight_request,
                *preflight_request.session_times.end(),
                Timestamp::now(),
            )
            .await?;
        if !acquired {
            return Err(SourceChainError::ChainLocked);
        }

        Ok(countersigning_agent_state)
    }

    pub async fn put_with_action(
        &self,
        action: Action,
        maybe_entry: Option<Entry>,
        chain_top_ordering: ChainTopOrdering,
    ) -> SourceChainResult<ActionHash> {
        let action = ActionHashed::from_content_sync(action);
        let hash = action.as_hash().clone();
        let action = sign_legacy_action(&self.keystore, action).await?;
        let record = Record::new(action, maybe_entry);
        self.scratch
            .apply(|scratch| insert_record_scratch(scratch, record, chain_top_ordering))?;
        Ok(hash)
    }

    pub async fn put_countersigned(
        &self,
        entry: Entry,
        chain_top_ordering: ChainTopOrdering,
        weight: EntryRateWeight,
    ) -> SourceChainResult<ActionHash> {
        let entry_hash = EntryHash::with_data_sync(&entry);
        if let Entry::CounterSign(ref session_data, _) = entry {
            self.put_with_action(
                Action::from_countersigning_data(
                    entry_hash,
                    session_data,
                    (*self.author).clone(),
                    weight,
                )?,
                Some(entry),
                chain_top_ordering,
            )
            .await
        } else {
            // The caller MUST guard against this case.
            unreachable!("Put countersigned called with the wrong entry type");
        }
    }

    /// Put a new record at the end of the source chain, using a ActionBuilder
    /// for an action type which has no weight data.
    /// If needing to `put` an action with weight data, use
    /// [`SourceChain::put_weighed`] instead.
    pub async fn put<U: ActionUnweighed<Weight = ()>, B: ActionBuilder<U>>(
        &self,
        action_builder: B,
        maybe_entry: Option<Entry>,
        chain_top_ordering: ChainTopOrdering,
    ) -> SourceChainResult<ActionHash> {
        self.put_weighed(action_builder, maybe_entry, chain_top_ordering, ())
            .await
    }

    /// Put a new record at the end of the source chain, using a ActionBuilder
    /// and the specified weight for rate limiting.
    pub async fn put_weighed<W, U: ActionUnweighed<Weight = W>, B: ActionBuilder<U>>(
        &self,
        action_builder: B,
        maybe_entry: Option<Entry>,
        chain_top_ordering: ChainTopOrdering,
        weight: W,
    ) -> SourceChainResult<ActionHash> {
        let HeadInfo {
            action: prev_action,
            seq: chain_head_seq,
            timestamp: chain_head_timestamp,
        } = self.chain_head_nonempty()?;
        let action_seq = chain_head_seq + 1;

        // Build the action.
        let common = ActionBuilderCommon {
            author: (*self.author).clone(),
            // If the current time is equal to the current chain head timestamp,
            // or even has drifted to be before it, just set the next timestamp
            // to be one unit ahead of the previous.
            //
            // TODO: put a limit on the size of the negative time interval
            //       we are willing to accept, beyond which we emit an error
            //       rather than bumping the timestamp
            timestamp: std::cmp::max(
                Timestamp::now(),
                (chain_head_timestamp + std::time::Duration::from_micros(1))?,
            ),
            action_seq,
            prev_action,
        };
        self.put_with_action(
            action_builder.build(common).weighed(weight).into(),
            maybe_entry,
            chain_top_ordering,
        )
        .await
    }

    // TODO: when we fully hook up rate limiting, make this test-only
    // #[cfg(feature = "test_utils")]
    pub async fn put_weightless<W: Default, U: ActionUnweighed<Weight = W>, B: ActionBuilder<U>>(
        &self,
        action_builder: B,
        maybe_entry: Option<Entry>,
        chain_top_ordering: ChainTopOrdering,
    ) -> SourceChainResult<ActionHash> {
        self.put_weighed(
            action_builder,
            maybe_entry,
            chain_top_ordering,
            Default::default(),
        )
        .await
    }

    /// Drain the scratch space and persist its contents to the databases.
    ///
    /// This drains all actions, entries, scheduled functions, and warrants from the scratch
    /// and writes them to the authored and DHT databases. The flush proceeds as follows:
    ///
    /// 1. Validates countersigning invariants: a countersigning entry must be the only
    ///    action in the scratch, its chain lock must match and not be expired.
    /// 2. Performs an "as-at" consistency check against the current persisted chain head.
    ///    If the head has moved since this [`SourceChain`] was constructed, the behavior
    ///    depends on the [`ChainTopOrdering`]: under [`ChainTopOrdering::Strict`] the
    ///    flush fails with [`SourceChainError::HeadMoved`]; under
    ///    [`ChainTopOrdering::Relaxed`] the actions are rebased onto the new head and
    ///    the flush is retried recursively.
    /// 3. Inserts entries, actions, and ops into the DhtStore in a single
    ///    transaction. Ops belonging to a countersigning session are marked as withheld
    ///    from publishing.
    /// 4. Verifies warrant signatures and records valid warrants into the DhtStore.
    ///
    /// Returns an empty vec with zero warrants if the scratch is empty.
    ///
    /// # Errors
    ///
    /// - [`SourceChainError::HeadMoved`] if the chain head changed and ordering is strict.
    /// - [`SourceChainError::DirtyCounterSigningWrite`] if a countersigning entry is
    ///   flushed alongside other actions.
    /// - [`SourceChainError::ChainLocked`] if the chain is locked for a different session.
    /// - [`SourceChainError::LockExpired`] if the countersigning lock has expired.
    /// - [`SourceChainError::CountersigningWriteWithoutSession`] if a countersigning
    ///   entry is written without an active chain lock.
    #[async_recursion]
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
    // `storage_arcs` is only used in the recursive rebase call.
    #[allow(clippy::only_used_in_recursion)]
    pub async fn flush(
        &self,
        storage_arcs: Vec<DhtArc>,
    ) -> SourceChainResult<(Vec<SignedActionHashed>, u32)> {
        // Nothing to write
        if self.scratch.apply(|s| s.is_empty())? {
            return Ok((Vec::new(), 0));
        }

        let (scheduled_fns, actions, ops, entries, records, warrants) =
            self.scratch.apply_and_then(|scratch| {
                let records: Vec<Record> = scratch.records().collect();

                let (actions, ops) =
                    build_ops_from_actions(scratch.drain_actions().collect::<Vec<_>>())?;

                // Drain out any entries.
                let entries = scratch.drain_entries().collect::<Vec<_>>();
                let scheduled_fns = scratch.drain_scheduled_fns().collect::<Vec<_>>();
                let warrants = scratch.drain_warrants().collect::<Vec<_>>();
                SourceChainResult::Ok((scheduled_fns, actions, ops, entries, records, warrants))
            })?;

        let maybe_countersigned_entry = entries
            .iter()
            .map(|entry| entry.as_content())
            .find(|entry| matches!(entry, Entry::CounterSign(_, _)));

        if matches!(maybe_countersigned_entry, Some(Entry::CounterSign(_, _))) && actions.len() != 1
        {
            return Err(SourceChainError::DirtyCounterSigningWrite);
        }

        let lock_subject = chain_lock_subject_for_entry(maybe_countersigned_entry)?;

        // If the lock isn't empty this is a countersigning session.
        let is_countersigning_session = !lock_subject.is_empty();

        let author = self.author.clone();
        let persisted_head = self.head_info.as_ref().map(|h| h.action.clone());

        let now = Timestamp::now();

        // Acquire the per-author chain write permit on the DhtStore and
        // perform the source-chain write under it, gated by an as-at check
        // against the store head. The permit serializes flushes for this
        // (DNA, author) chain so a concurrent flush cannot also pass the
        // as-at and fork the chain.
        //
        // The permit is acquired and released *inside* this async block so it
        // is not held when the relaxed-ordering rebase below recurses into
        // `flush` (which re-acquires it).
        let chain_flush_result: SourceChainResult<Vec<SignedActionHashed>> = async {
            let _chain_write_permit = self
                .dht_store
                .acquire_chain_write_permit(author.as_ref())
                .await;

            // If there are records to write, then we need to respect the chain
            // lock. Reading it here opens a tiny TOCTOU window (the lock is
            // mutated by the countersigning workflow, not by flush, so the
            // chain permit does not stabilise it), but this is a coarse
            // countersigning-session guard and the window is acceptable.
            if !records.is_empty() {
                let chain_lock = self
                    .dht_store
                    .as_read()
                    .get_chain_lock(author.as_ref().clone())
                    .await?;
                match chain_lock {
                    Some(chain_lock) => {
                        // If the chain is locked, the lock must be for this entry.
                        if chain_lock.subject() != lock_subject {
                            return Err(SourceChainError::ChainLocked);
                        }
                        // If the lock is expired then we can't write this countersigning session.
                        else if chain_lock.is_expired_at(now) {
                            return Err(SourceChainError::LockExpired);
                        }

                        // Otherwise, the lock matches this entry and has not expired. We can proceed!
                    }
                    None => {
                        // If this is a countersigning entry but there is no chain lock then maybe
                        // the session expired before the entry could be written or maybe the app
                        // has just made a mistake. Either way, it's not valid to write this entry!
                        if is_countersigning_session {
                            return Err(SourceChainError::CountersigningWriteWithoutSession);
                        }
                    }
                }
            }

            // As-at check against the STORE head, under the permit, before the
            // write. Only meaningful when there are actions that move the head.
            // The permit guarantees no concurrent flush commits between this
            // read and the write below, and no other code path writes this
            // author's chain head, so reading on a pool connection (rather than
            // inside `tx`) is safe.
            if !actions.is_empty() {
                let head_info = self
                    .dht_store
                    .as_read()
                    .chain_head_for_author(author.as_ref())
                    .await?;
                let latest_head = head_info.as_ref().map(|h| h.action.clone());

                if persisted_head != latest_head {
                    return Err(SourceChainError::HeadMoved(
                        Box::new(actions),
                        Box::new(entries),
                        persisted_head,
                        head_info,
                    ));
                }
            }

            // The authoritative source-chain write: entries, actions, ops and
            // scheduled fns into the DhtStore, in one transaction.
            let mut tx = self
                .dht_store
                .db()
                .begin()
                .await
                .map_err(SourceChainError::other)?;

            // Collect the set of entry hashes whose authoring action declares
            // them as private. Entries whose hash matches one of these go to
            // `PrivateEntry`; every other entry — including any whose hash is
            // not referenced by an in-batch action — goes to the public `Entry`
            // table.
            let private_entry_hashes = actions
                .iter()
                .filter_map(|sah| {
                    let action = sah.action();
                    let visibility = action.entry_visibility()?;
                    if *visibility == EntryVisibility::Private {
                        action.entry_hash().cloned()
                    } else {
                        None
                    }
                })
                .collect::<std::collections::HashSet<_>>();

            for entry_hashed in &entries {
                let entry_hash = entry_hashed.as_hash();
                let entry = entry_hashed.as_content();
                if private_entry_hashes.contains(entry_hash) {
                    tx.insert_private_entry(entry_hash, author.as_ref(), entry)
                        .await
                        .map_err(SourceChainError::other)?;
                } else {
                    tx.insert_entry(entry_hash, entry)
                        .await
                        .map_err(SourceChainError::other)?;
                }
            }

            // Track which action hashes were successfully inserted into the
            // store. Ops whose action insert failed must also be skipped to
            // avoid FK violations on ChainOp.action_hash.
            let mut inserted_action_hashes = std::collections::HashSet::<ActionHash>::new();

            for sah in &actions {
                let new_sah = holochain_zome_types::dht_v2::from_legacy_signed_action(sah);

                tx.insert_action(
                    &new_sah,
                    Some(holochain_zome_types::dht_v2::RecordValidity::Accepted),
                )
                .await
                .map_err(SourceChainError::other)?;

                // Record that this action hash is present in the store.
                inserted_action_hashes.insert(sah.as_hash().clone());

                crate::dht_store::action_indexes::insert_action_indexes(
                    &mut tx,
                    new_sah.as_hash(),
                    &new_sah.hashed.content.data,
                )
                .await
                .map_err(SourceChainError::other)?;

                // For Create/Update of a CapGrant entry type, insert a CapGrant index row.
                if let Some((cap_access, tag)) = cap_grant_index_params(sah, &entries) {
                    tx.insert_cap_grant(new_sah.as_hash(), cap_access, tag.as_deref())
                        .await
                        .map_err(SourceChainError::other)?;
                }
            }

            for (op, op_hash, _op_order, timestamp, _dep) in &ops {
                let op_as_chain = match op.as_chain_op() {
                    Some(c) => c,
                    None => continue, // warrant ops: skip for this slice
                };
                // Skip ops whose action hash was not recorded as successfully inserted.
                if !inserted_action_hashes.contains(op_as_chain.action_hash()) {
                    continue;
                }
                let basis_hash = op_as_chain.dht_basis().clone();
                let storage_center_loc = basis_hash.get_loc();

                let serialized_size = encoded_chain_op_size(op_as_chain, &actions, &entries);
                tx.insert_chain_op(holochain_data::dht::InsertChainOp {
                    op_hash,
                    action_hash: op_as_chain.action_hash(),
                    op_type: i64::from(op_as_chain.get_type()),
                    basis_hash: &basis_hash,
                    storage_center_loc,
                    validation_status: holochain_zome_types::dht_v2::RecordValidity::Accepted,
                    locally_validated: true,
                    require_receipt: false,
                    when_received: *timestamp,
                    when_integrated: *timestamp,
                    serialized_size,
                })
                .await
                .map_err(SourceChainError::other)?;

                // Always insert a ChainOpPublish row for self-authored ops so the
                // publish workflow can track them without a separate lookup.
                // Countersigning ops are withheld from publishing until the
                // session succeeds.
                let withhold = if is_countersigning_session {
                    Some(true)
                } else {
                    None
                };
                tx.insert_chain_op_publish(op_hash, None, None, withhold)
                    .await
                    .map_err(SourceChainError::other)?;
            }

            // Scheduled functions flushed from the scratch are always written
            // with `maybe_schedule = None`.
            // None => start=now, end=Timestamp::max(), ephemeral=true.
            for scheduled_fn in &scheduled_fns {
                let maybe_schedule_blob =
                    serialize_maybe_schedule_none().map_err(SourceChainError::other)?;
                let _ = tx
                    .upsert_scheduled_function(holochain_data::dht::InsertScheduledFunction {
                        author: author.as_ref(),
                        zome_name: scheduled_fn.zome_name().0.as_ref(),
                        scheduled_fn: scheduled_fn.fn_name().0.as_ref(),
                        maybe_schedule: &maybe_schedule_blob,
                        start_at: now,
                        end_at: Timestamp::max(),
                        ephemeral: true,
                    })
                    .await
                    .map_err(SourceChainError::other)?;
            }

            tx.commit().await.map_err(SourceChainError::other)?;

            SourceChainResult::Ok(actions)
        }
        .await;

        match chain_flush_result {
            Err(SourceChainError::HeadMoved(actions, entries, old_head, Some(new_head_info))) => {
                let is_relaxed =
                    self.scratch
                        .apply_and_then::<bool, SyncScratchError, _>(|scratch| {
                            Ok(scratch.chain_top_ordering() == ChainTopOrdering::Relaxed)
                        })?;
                if is_relaxed {
                    let keystore = self.keystore.clone();
                    // A child chain is needed with a new as-at that matches
                    // the rebase.
                    let child_chain = Self::new(
                        self.dht_store.clone(),
                        keystore.clone(),
                        (*self.author).clone(),
                    )
                    .await?;
                    let rebased_actions =
                        rebase_actions_on(&keystore, *actions, new_head_info).await?;
                    child_chain.scratch.apply(move |scratch| {
                        for action in rebased_actions {
                            scratch.add_action(action, ChainTopOrdering::Relaxed);
                        }
                        for entry in *entries {
                            scratch.add_entry(entry, ChainTopOrdering::Relaxed);
                        }
                    })?;
                    child_chain.flush(storage_arcs).await
                } else {
                    Err(SourceChainError::HeadMoved(
                        actions,
                        entries,
                        old_head,
                        Some(new_head_info),
                    ))
                }
            }
            Ok(actions) => {
                // Verify warrant signatures, then record the valid ones into the
                // DhtStore so `ops_pending_sys_validation` picks them up via
                // `LimboWarrant`.
                let mut warrant_ops = Vec::new();
                for warrant in warrants {
                    match warrant
                        .author
                        .verify_signature(warrant.signature(), warrant.data())
                        .await
                    {
                        Ok(true) => warrant_ops.push(DhtOpHashed::from_content_sync(DhtOp::from(
                            WarrantOp::from(warrant),
                        ))),
                        Ok(false) => {
                            tracing::info!(
                                "Invalid signature of a warrant in the scratch space. Skipping warrant"
                            );
                            continue;
                        }
                        Err(err) => {
                            tracing::warn!(?err, "Could not verify warrant signature before recording from scratch space into the DhtStore. Skipping warrant");
                            continue;
                        }
                    }
                }

                let total_warrants = warrant_ops.len() as u32;
                if !warrant_ops.is_empty() {
                    // Warrants do not require validation receipts, set all to false.
                    let warrant_ops_with_validation_receipt_required_flag =
                        warrant_ops.into_iter().map(|op| (op, false)).collect();
                    self.dht_store
                        .record_incoming_ops(warrant_ops_with_validation_receipt_required_flag)
                        .await
                        .map_err(SourceChainError::other)?;
                }

                SourceChainResult::Ok((actions, total_warrants))
            }
            Err(e) => Err(e),
        }
    }

    /// Checks if the current [`AgentPubKey`] of the source chain is valid and returns its [`Create`] action.
    ///
    /// Valid means that there's no [`Update`] or [`Delete`] action for the key on the chain.
    /// Returns the create action if it is valid, and an [`SourceChainError::InvalidAgentKey`] otherwise.
    ///
    /// Returns the v2 `Action` directly from [`DhtStore::valid_create_agent_key_action`]
    /// (the source of truth) rather than converting to the legacy shape this
    /// module otherwise uses for authoring — this is a read, not a step in
    /// building a new action, and the returned action's hash (used by
    /// [`Self::delete_valid_agent_pub_key`]) is identical either way.
    pub async fn valid_create_agent_key_action(
        &self,
    ) -> SourceChainResult<holochain_zome_types::prelude::Action> {
        let agent_key = self.agent_pubkey().clone();
        self.dht_store
            .as_read()
            .valid_create_agent_key_action(&agent_key)
            .await?
            .ok_or_else(|| {
                SourceChainError::InvalidAgentKey(agent_key, self.cell_id().as_ref().clone())
            })
    }

    /// Deletes the current [`AgentPubKey`] of the source chain if it is valid and returns a [`SourceChainError::InvalidAgentKey`]
    /// otherwise.
    ///
    /// The agent key is valid if there are no [`Update`] or [`Delete`] actions for that key on the chain.
    pub async fn delete_valid_agent_pub_key(&self) -> SourceChainResult<()> {
        let valid_create_agent_key_action = self.valid_create_agent_key_action().await?;

        self.put_weightless(
            builder::Delete::new(
                valid_create_agent_key_action.to_hash(),
                self.agent_pubkey().clone().into(),
            ),
            None,
            ChainTopOrdering::Strict,
        )
        .await?;

        Ok(())
    }
}

impl SourceChain<DbWrite<Dht>> {
    pub async fn new(
        dht_store: DhtStore,
        keystore: MetaLairClient,
        author: AgentPubKey,
    ) -> SourceChainResult<Self> {
        let scratch = Scratch::new().into_sync();
        let author = Arc::new(author);
        let cell_id = Arc::new(CellId::new(
            dht_store.dna_hash().clone(),
            author.as_ref().clone(),
        ));
        let head_info = Some(
            dht_store
                .as_read()
                .chain_head_for_author(author.as_ref())
                .await?
                .ok_or(SourceChainError::ChainEmpty)?,
        );
        Ok(Self {
            scratch,
            dht_store,
            keystore,
            author,
            cell_id,
            head_info,
            public_only: false,
            zomes_initialized: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Create a source chain with a blank chain head.
    /// You probably don't want this.
    /// This type is only useful for when a source chain
    /// really needs to be constructed before genesis runs.
    pub async fn raw_empty(
        dht_store: DhtStore,
        keystore: MetaLairClient,
        author: AgentPubKey,
    ) -> SourceChainResult<Self> {
        let scratch = Scratch::new().into_sync();
        let author = Arc::new(author);
        let cell_id = Arc::new(CellId::new(
            dht_store.dna_hash().clone(),
            author.as_ref().clone(),
        ));
        let head_info = dht_store
            .as_read()
            .chain_head_for_author(author.as_ref())
            .await?;
        Ok(Self {
            scratch,
            dht_store,
            keystore,
            author,
            cell_id,
            head_info,
            public_only: false,
            zomes_initialized: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Downgrade this writable source chain to a read-only source chain.
    pub fn as_read(&self) -> SourceChainRead {
        SourceChain {
            scratch: self.scratch.clone(),
            dht_store: self.dht_store.as_read(),
            keystore: self.keystore.clone(),
            author: self.author.clone(),
            cell_id: self.cell_id.clone(),
            head_info: self.head_info.clone(),
            public_only: self.public_only,
            zomes_initialized: self.zomes_initialized.clone(),
        }
    }
}

impl<Db> SourceChain<Db>
where
    Db: AsRef<DbRead<Dht>>,
{
    pub fn public_only(&mut self) {
        self.public_only = true;
    }

    pub fn keystore(&self) -> &MetaLairClient {
        &self.keystore
    }

    /// Take a snapshot of the scratch space that will
    /// not remain in sync with future updates.
    pub fn snapshot(&self) -> SourceChainResult<Scratch> {
        Ok(self.scratch.apply(|scratch| scratch.clone())?)
    }

    pub fn scratch(&self) -> SyncScratch {
        self.scratch.clone()
    }

    pub fn agent_pubkey(&self) -> &AgentPubKey {
        self.author.as_ref()
    }

    pub fn to_agent_pubkey(&self) -> Arc<AgentPubKey> {
        self.author.clone()
    }

    pub fn cell_id(&self) -> Arc<CellId> {
        self.cell_id.clone()
    }

    /// This has to clone all the data because we can't return
    /// references to constructed data.
    // TODO: Maybe we should store data as records in the scratch?
    // TODO: document that this is only the records in the SCRATCH, not the
    //       entire source chain!
    pub fn scratch_records(&self) -> SourceChainResult<Vec<Record>> {
        Ok(self.scratch.apply(|scratch| scratch.records().collect())?)
    }

    pub async fn zomes_initialized(&self) -> SourceChainResult<bool> {
        if self.zomes_initialized.load(Ordering::Relaxed) {
            return Ok(true);
        }
        let query_filter = ChainQueryFilter {
            action_type: Some(vec![ActionType::InitZomesComplete]),
            ..QueryFilter::default()
        };
        let init_zomes_complete_actions = self.query(query_filter).await?;
        if init_zomes_complete_actions.len() > 1 {
            tracing::warn!("Multiple InitZomesComplete actions are present");
        }
        let zomes_initialized = !init_zomes_complete_actions.is_empty();
        self.set_zomes_initialized(zomes_initialized);
        Ok(zomes_initialized)
    }

    pub fn set_zomes_initialized(&self, value: bool) {
        self.zomes_initialized.store(value, Ordering::Relaxed);
    }

    /// Accessor for the chain head that will be used at flush time to check
    /// the "as at" for ordering integrity etc.
    pub fn persisted_head_info(&self) -> Option<HeadInfo> {
        self.head_info.clone()
    }

    pub fn chain_head(&self) -> SourceChainResult<Option<HeadInfo>> {
        // Check scratch for newer head.
        Ok(self
            .scratch
            .apply(|scratch| scratch.chain_head().or_else(|| self.persisted_head_info()))?)
    }

    pub fn chain_head_nonempty(&self) -> SourceChainResult<HeadInfo> {
        // Check scratch for newer head.
        self.chain_head()?.ok_or(SourceChainError::ChainEmpty)
    }

    #[cfg(feature = "test_utils")]
    pub fn len(&self) -> SourceChainResult<u32> {
        Ok(self.scratch.apply(|scratch| {
            let scratch_max = scratch.chain_head().map(|h| h.seq);
            let persisted_max = self.head_info.as_ref().map(|h| h.seq);
            match (scratch_max, persisted_max) {
                (None, None) => 0,
                (Some(s), None) => s + 1,
                (None, Some(s)) => s + 1,
                (Some(a), Some(b)) => a.max(b) + 1,
            }
        })?)
    }

    #[cfg(feature = "test_utils")]
    pub fn is_empty(&self) -> SourceChainResult<bool> {
        Ok(self.len()? == 0)
    }

    pub async fn valid_cap_grant(
        &self,
        check_function: GrantedFunction,
        check_agent: AgentPubKey,
        check_secret: Option<CapSecret>,
    ) -> SourceChainResult<Option<CapGrant>> {
        let author_grant = CapGrant::from(self.agent_pubkey().clone());
        if author_grant.is_valid(&check_function, &check_agent, check_secret.as_ref()) {
            // caller is source chain author
            return Ok(Some(author_grant));
        }

        // Remote caller. The candidate grants are read from the DhtStore, which
        // applies the access-type pre-filter and "not updated/deleted"
        // exclusion; the exact secret/assignee/function match remains the
        // authority of `CapGrant::is_valid` below.
        let cap_grants = self
            .dht_store
            .as_read()
            .valid_cap_grants(self.agent_pubkey(), check_secret.as_ref())
            .await?;
        // Loop over all found cap grants and check if one of them is valid for
        // assignee and function.
        for cap_grant in cap_grants {
            if cap_grant.is_valid(&check_function, &check_agent, check_secret.as_ref()) {
                return Ok(Some(cap_grant));
            }
        }
        Ok(None)
    }

    /// Query Actions in the source chain.
    ///
    /// This returns a Vec rather than an iterator because it is intended to be
    /// used by the `query` host function, which crosses the wasm boundary.
    ///
    /// Returns v2 [`holochain_zome_types::prelude::Record`]s, matching
    /// [`DhtStore::source_chain_records`]. The scratch (this module's LEGACY
    /// ISLAND) is overlaid afterwards; each staged legacy action is converted
    /// to v2 with [`holochain_zome_types::dht_v2::from_legacy_signed_action`]
    /// before merging into the v2 result — the one cross-boundary point in
    /// this file.
    pub async fn query(
        &self,
        query: QueryFilter,
    ) -> SourceChainResult<Vec<holochain_zome_types::prelude::Record>> {
        let public_only = self.public_only;

        // Fetch the author's committed records from the DhtStore (no filtering
        // applied here). Ordering and filtering are handled below;
        // `ChainQueryFilter::filter_records` is the final authority.
        let mut records = self
            .dht_store
            .as_read()
            .source_chain_records(self.author.as_ref(), query.include_entries, public_only)
            .await?;

        // The store returns committed records in ascending sequence order.
        // `order_descending` applies to the committed records only (the scratch
        // is always appended in ascending order below), so reverse the
        // committed records for a descending query.
        if query.order_descending {
            records.reverse();
        }

        // Just take anything from the scratch for now. More filtering is possibly needed against
        // the results returned from the database anyway.
        self.scratch.apply(|scratch| {
            let mut scratch_records: Vec<_> = scratch
                .actions()
                .filter_map(|sah| {
                    let entry = match sah.action().entry_hash() {
                        Some(eh) if query.include_entries => scratch.get_entry(eh).ok()?,
                        _ => None,
                    };
                    let v2_sah = holochain_zome_types::dht_v2::from_legacy_signed_action(sah);
                    let record_entry = holochain_zome_types::record::RecordEntry::new(
                        sah.action().entry_visibility(),
                        entry,
                    );
                    Some(holochain_zome_types::prelude::Record::new(
                        v2_sah,
                        record_entry,
                    ))
                })
                .collect();
            scratch_records.sort_unstable_by_key(|e| e.action().header.action_seq);
            records.extend(scratch_records);
        })?;

        Ok(query.filter_records(records))
    }

    pub async fn get_chain_lock(&self) -> SourceChainResult<Option<ChainLock>> {
        // The chain lock lives in the DhtStore.
        Ok(self
            .dht_store
            .as_read()
            .get_chain_lock(self.author.as_ref().clone())
            .await?)
    }

    /// If there is a countersigning session get the
    /// StoreEntry op to send to the entry authorities.
    pub fn countersigning_op(&self) -> SourceChainResult<Option<ChainOp>> {
        let r = self.scratch.apply(|scratch| {
            scratch
                .entries()
                .find(|e| matches!(**e.1, Entry::CounterSign(_, _)))
                .and_then(|(entry_hash, entry)| {
                    scratch
                        .actions()
                        .find(|shh| {
                            shh.action()
                                .entry_hash()
                                .map(|eh| eh == entry_hash)
                                .unwrap_or(false)
                        })
                        .and_then(|shh| {
                            Some(ChainOp::StoreEntry(
                                shh.signature().clone(),
                                shh.action().clone().try_into().ok()?,
                                (**entry).clone(),
                            ))
                        })
                })
        })?;
        Ok(r)
    }

    pub async fn dump(&self) -> SourceChainResult<SourceChainDump> {
        dump_state(&self.dht_store.as_read(), (*self.author).clone()).await
    }
}

pub fn chain_lock_subject_for_entry(entry: Option<&Entry>) -> SourceChainResult<Vec<u8>> {
    Ok(match entry {
        // TODO document that this implies preflight requests must be unique. I.e. if you want to countersign the
        //      same thing with multiple groups, then you need to use different session times.
        Some(Entry::CounterSign(session_data, _)) => holo_hash::encode::blake2b_256(
            &holochain_serialized_bytes::encode(session_data.preflight_request())?,
        ),
        _ => Vec::with_capacity(0),
    })
}

#[allow(clippy::complexity)]
fn build_ops_from_actions(
    actions: Vec<SignedActionHashed>,
) -> SourceChainResult<(
    Vec<SignedActionHashed>,
    Vec<(DhtOpLite, DhtOpHash, OpOrder, Timestamp, Vec<ActionHash>)>,
)> {
    // Actions end up back in here.
    let mut actions_output = Vec::with_capacity(actions.len());
    // The op related data ends up here.
    let mut ops = Vec::with_capacity(actions.len());

    // Loop through each action and produce op related data.
    for shh in actions {
        // &ActionHash, &Action, EntryHash are needed to produce the ops.
        let entry_hash = shh.action().entry_hash().cloned();
        let item = (shh.as_hash(), shh.action(), entry_hash);
        let ops_inner = produce_op_lites_from_iter(vec![item].into_iter())?;

        // Break apart the SignedActionHashed.
        let (action, sig) = shh.into_inner();
        let (action, hash) = action.into_inner();

        // We need to take the action by value and put it back each loop.
        let mut h = Some(action);
        for op in ops_inner {
            let op_type = op.get_type();
            let op = DhtOpLite::from(op);
            // Action is required by value to produce the DhtOpHash.
            let (action, op_hash) =
                ChainOpUniqueForm::op_hash(op_type, h.expect("This can't be empty"))?;
            let op_order = OpOrder::new(op_type, action.timestamp());
            let timestamp = action.timestamp();
            // Put the action back by value.
            let deps = op_type.sys_validation_dependencies(&action);
            h = Some(action);
            // Collect the DhtOpLite, DhtOpHash and OpOrder.
            ops.push((op, op_hash, op_order, timestamp, deps));
        }

        // Put the SignedActionHashed back together.
        let shh = SignedActionHashed::with_presigned(
            ActionHashed::with_pre_hashed(h.expect("This can't be empty"), hash),
            sig,
        );
        // Put the action back in the list.
        actions_output.push(shh);
    }
    Ok((actions_output, ops))
}

/// Sign a hashed legacy action, computing the signature over the v2
/// projection of its content.
///
/// The authoring pipeline stays on the legacy `Action` shape until Phase 5,
/// so `action_hashed` carries the legacy, per-variant content (its hash is
/// already the content-derived v2 hash). The signature itself is computed
/// over `from_legacy_action(&action_hashed.content)` — the same v2 bytes
/// every verifier in the system (`holochain::core::sys_validate::
/// verify_action_signature` and the cascade's rendered-op / activity
/// signature checks) projects to and checks against.
async fn sign_legacy_action(
    keystore: &MetaLairClient,
    action_hashed: ActionHashed,
) -> holochain_keystore::LairResult<SignedActionHashed> {
    let v2 = holochain_zome_types::dht_v2::from_legacy_action(&action_hashed.content);
    let signature = action_hashed.content.signer().sign(keystore, &v2).await?;
    Ok(SignedActionHashed::with_presigned(action_hashed, signature))
}

/// Rebase a legacy action onto a new chain head, updating its
/// timestamp/seq/prev_action fields in place.
///
/// Mirrors `holochain_zome_types::action::ActionExt::rebase_on`, which only
/// implements this for the v2 `Action` shape; the authoring pipeline stays
/// on the legacy per-variant shape until Phase 5, so this operates on the
/// legacy fields directly.
fn rebase_legacy_action(
    action: &mut Action,
    new_prev_action: ActionHash,
    new_prev_seq: u32,
    new_prev_timestamp: Timestamp,
) -> Result<(), ActionError> {
    if matches!(action, Action::Dna(_)) {
        return Err(ActionError::Rebase("Rebased a DNA Action".to_string()));
    }
    let new_seq = new_prev_seq + 1;
    let new_timestamp = action.timestamp().max(
        (new_prev_timestamp + std::time::Duration::from_nanos(1))
            .map_err(|e| ActionError::Rebase(e.to_string()))?,
    );
    macro_rules! set_common_fields {
        ($i:ident) => {{
            $i.timestamp = new_timestamp;
            $i.action_seq = new_seq;
            $i.prev_action = new_prev_action;
        }};
    }
    match action {
        Action::Dna(_) => unreachable!("DNA action rejected above"),
        Action::AgentValidationPkg(a) => set_common_fields!(a),
        Action::InitZomesComplete(a) => set_common_fields!(a),
        Action::CreateLink(a) => set_common_fields!(a),
        Action::DeleteLink(a) => set_common_fields!(a),
        Action::CloseChain(a) => set_common_fields!(a),
        Action::OpenChain(a) => set_common_fields!(a),
        Action::Create(a) => set_common_fields!(a),
        Action::Update(a) => set_common_fields!(a),
        Action::Delete(a) => set_common_fields!(a),
    }
    Ok(())
}

async fn rebase_actions_on(
    keystore: &MetaLairClient,
    mut actions: Vec<SignedActionHashed>,
    mut head: HeadInfo,
) -> Result<Vec<SignedActionHashed>, ScratchError> {
    actions.sort_by_key(|shh| shh.action().action_seq());
    for shh in actions.iter_mut() {
        let mut action = shh.action().clone();
        rebase_legacy_action(&mut action, head.action.clone(), head.seq, head.timestamp)?;
        head.seq = action.action_seq();
        head.timestamp = action.timestamp();
        let hh = ActionHashed::from_content_sync(action);
        head.action = hh.as_hash().clone();
        let new_shh = sign_legacy_action(keystore, hh).await?;
        *shh = new_shh;
    }
    Ok(actions)
}

#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub async fn genesis(
    dht_store: DhtStore,
    keystore: MetaLairClient,
    dna_hash: DnaHash,
    agent_pubkey: AgentPubKey,
    membrane_proof: Option<MembraneProof>,
) -> SourceChainResult<()> {
    let dna_action = Action::Dna(Dna {
        author: agent_pubkey.clone(),
        timestamp: Timestamp::now(),
        hash: dna_hash,
    });
    let dna_action = ActionHashed::from_content_sync(dna_action);
    let dna_action = sign_legacy_action(&keystore, dna_action).await?;
    let dna_action_address = dna_action.as_hash().clone();
    let dna_record = Record::new(dna_action, None);
    let dna_ops = produce_op_lites_from_records(vec![&dna_record])?;
    let (dna_action, _) = dna_record.clone().into_inner();

    // create the agent validation entry and add it directly to the store
    let agent_validation_action = Action::AgentValidationPkg(AgentValidationPkg {
        author: agent_pubkey.clone(),
        timestamp: Timestamp::now(),
        action_seq: 1,
        prev_action: dna_action_address,
        membrane_proof,
    });
    let agent_validation_action = ActionHashed::from_content_sync(agent_validation_action);
    let agent_validation_action = sign_legacy_action(&keystore, agent_validation_action).await?;
    let avh_addr = agent_validation_action.as_hash().clone();
    let agent_validation_record = Record::new(agent_validation_action, None);
    let avh_ops = produce_op_lites_from_records(vec![&agent_validation_record])?;
    let (agent_validation_action, _) = agent_validation_record.clone().into_inner();

    // create a agent chain record and add it directly to the store
    let agent_action = Action::Create(Create {
        author: agent_pubkey.clone(),
        timestamp: Timestamp::now(),
        action_seq: 2,
        prev_action: avh_addr,
        entry_type: EntryType::AgentPubKey,
        entry_hash: agent_pubkey.clone().into(),
        // AgentPubKey is weightless
        weight: Default::default(),
    });
    let agent_action = ActionHashed::from_content_sync(agent_action);
    let agent_action = sign_legacy_action(&keystore, agent_action).await?;
    let agent_record = Record::new(agent_action, Some(Entry::Agent(agent_pubkey.clone())));
    let agent_ops = produce_op_lites_from_records(vec![&agent_record])?;
    let (agent_action, agent_entry) = agent_record.clone().into_inner();
    let agent_entry = agent_entry.into_option();

    // Pre-compute (op, op_hash, timestamp) tuples for the DhtStore write block.
    // Each op's hash is derived via ChainOpUniqueForm::op_hash upfront so we can
    // keep the actions and ops available for the DhtStore write without moving
    // them into the closure.
    //
    // Each triple is (ChainOpLite, DhtOpHash, Timestamp) for one op.
    let mut ops_with_hashes_for_new_db: Vec<(ChainOpLite, DhtOpHash, Timestamp)> = Vec::new();
    {
        // Process each (signed_action, ops) pair to compute op hashes.
        // We clone the action here (cheap because Action uses Arc<[u8]> for large fields)
        // and let ChainOpUniqueForm::op_hash consume the clone.
        let pairs: &[(&SignedActionHashed, &[ChainOpLite])] = &[
            (&dna_action, &dna_ops),
            (&agent_validation_action, &avh_ops),
            (&agent_action, &agent_ops),
        ];
        for (shh, ops) in pairs {
            let mut action_opt: Option<Action> = Some(shh.action().clone());
            for op in *ops {
                let op_type = op.get_type();
                let (action_back, op_hash) = ChainOpUniqueForm::op_hash(
                    op_type,
                    action_opt.take().expect("action must be present"),
                )
                .map_err(SourceChainError::other)?;
                let timestamp = action_back.timestamp();
                action_opt = Some(action_back);
                ops_with_hashes_for_new_db.push(((*op).clone(), op_hash, timestamp));
            }
        }
    }

    // Clone the actions and agent entry for the DhtStore write block below.
    let dna_action_for_new_db = dna_action.clone();
    let agent_validation_action_for_new_db = agent_validation_action.clone();
    let agent_action_for_new_db = agent_action.clone();
    // `agent_entry` is `Option<Entry>`; clone it for the DhtStore write block.
    let agent_entry_for_new_db = agent_entry.clone();
    // Entry hash for the agent entry (AgentPubKey → EntryHash via Into).
    let agent_entry_hash: EntryHash = agent_pubkey.into();

    // Write the genesis actions, entries and ops to the DhtStore.
    {
        let mut tx = dht_store
            .db()
            .begin()
            .await
            .map_err(SourceChainError::other)?;

        // Insert the public agent entry (Entry::Agent is always public).
        if let Some(entry) = &agent_entry_for_new_db {
            tx.insert_entry(&agent_entry_hash, entry)
                .await
                .map_err(SourceChainError::other)?;
        }

        // Insert all three genesis actions.
        let genesis_actions: &[&SignedActionHashed] = &[
            &dna_action_for_new_db,
            &agent_validation_action_for_new_db,
            &agent_action_for_new_db,
        ];
        for sah in genesis_actions {
            let new_sah = holochain_zome_types::dht_v2::from_legacy_signed_action(sah);
            tx.insert_action(
                &new_sah,
                Some(holochain_zome_types::dht_v2::RecordValidity::Accepted),
            )
            .await
            .map_err(SourceChainError::other)?;
        }

        // Insert chain ops for all three genesis actions.
        for (op, op_hash, timestamp) in &ops_with_hashes_for_new_db {
            let basis_hash = op.dht_basis().clone();
            let storage_center_loc = basis_hash.get_loc();

            let genesis_actions_slice: Vec<SignedActionHashed> = vec![
                dna_action_for_new_db.clone(),
                agent_validation_action_for_new_db.clone(),
                agent_action_for_new_db.clone(),
            ];
            let genesis_entries_slice: Vec<holochain_types::EntryHashed> = agent_entry_for_new_db
                .as_ref()
                .map(|e| {
                    vec![holochain_types::EntryHashed::with_pre_hashed(
                        e.clone(),
                        agent_entry_hash.clone(),
                    )]
                })
                .unwrap_or_default();
            let serialized_size =
                encoded_chain_op_size(op, &genesis_actions_slice, &genesis_entries_slice);
            tx.insert_chain_op(holochain_data::dht::InsertChainOp {
                op_hash,
                action_hash: op.action_hash(),
                op_type: i64::from(op.get_type()),
                basis_hash: &basis_hash,
                storage_center_loc,
                validation_status: holochain_zome_types::dht_v2::RecordValidity::Accepted,
                locally_validated: true,
                require_receipt: false,
                when_received: *timestamp,
                when_integrated: *timestamp,
                serialized_size,
            })
            .await
            .map_err(SourceChainError::other)?;

            tx.insert_chain_op_publish(op_hash, None, None, None)
                .await
                .map_err(SourceChainError::other)?;
        }

        tx.commit().await.map_err(SourceChainError::other)?;
    }

    Ok(())
}

pub type CurrentCountersigningSessionOpt = Option<(Record, EntryHash, CounterSigningSessionData)>;

/// Dump the entire source chain from the DhtStore.
///
/// Private entries are included — the query looks in both the public `Entry`
/// table and the author's `PrivateEntry` table — so the dump faithfully
/// reflects the author's own chain. `published_ops_count` is the number of
/// integrated ops that have been published at least once.
///
/// This is the production path backing the admin `DumpState` and `DumpFullState`
/// APIs.
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub async fn dump_state(
    dht_store: &DhtStoreRead,
    author: AgentPubKey,
) -> Result<SourceChainDump, SourceChainError> {
    dht_store
        .dump_source_chain(&author)
        .await
        .map_err(SourceChainError::other)
}

// ---------------------------------------------------------------------------
// Private helpers for the new-DB writes in `flush` and `genesis`
// ---------------------------------------------------------------------------

/// Return the `(cap_access_i64, Option<tag>)` parameters needed for
/// `TxWrite::insert_cap_grant`, if the given action creates/updates a
/// `CapGrant` entry. Returns `None` for all other action types.
///
/// The entry content is needed to extract the tag; entries are looked up by
/// the entry hash carried by the action.
///
/// `Action`, `EntryType`, `CapAccess`, and `Entry` are all in scope via the
/// prelude.
fn cap_grant_index_params(
    shh: &SignedActionHashed,
    entries: &[EntryHashed],
) -> Option<(i64, Option<String>)> {
    let (entry_type, entry_hash) = match shh.action() {
        Action::Create(d) => (&d.entry_type, &d.entry_hash),
        Action::Update(d) => (&d.entry_type, &d.entry_hash),
        _ => return None,
    };

    if !matches!(entry_type, EntryType::CapGrant) {
        return None;
    }

    // Find the matching entry in the scratch batch.
    let entry = entries
        .iter()
        .find(|e| e.as_hash() == entry_hash)?
        .as_content();

    let cap_grant = match entry {
        Entry::CapGrant(g) => g,
        _ => return None,
    };

    let cap_access_i64 = match &cap_grant.access {
        CapAccess::Unrestricted => 0_i64,
        CapAccess::Transferable { .. } => 1_i64,
        CapAccess::Assigned { .. } => 2_i64,
    };
    // Deliberate empty→NULL normalisation: the schema stores an absent tag as
    // NULL rather than an empty string.
    let tag = if cap_grant.tag.is_empty() {
        None
    } else {
        Some(cap_grant.tag.clone())
    };

    Some((cap_access_i64, tag))
}

/// Serialize `None` as an `Option<Schedule>` blob.
///
/// `None` is serialized via
/// `holochain_serialized_bytes::encode(&None::<Schedule>)`.
fn serialize_maybe_schedule_none(
) -> Result<Vec<u8>, holochain_serialized_bytes::SerializedBytesError> {
    holochain_serialized_bytes::encode(&None::<holochain_zome_types::schedule::Schedule>)
}

/// Encode the wire-form `DhtOp` for a `ChainOpLite` and return its serialized
/// length in bytes. The action is looked up by hash in `actions`; the entry
/// (if any) is looked up by `Action::entry_hash` in `entries`. Returns `0`
/// only if the op cannot be reconstructed because the action is missing —
/// which would indicate a programming error in the caller.
pub(crate) fn encoded_chain_op_size(
    op: &holochain_types::dht_op::ChainOpLite,
    actions: &[SignedActionHashed],
    entries: &[holochain_types::EntryHashed],
) -> u32 {
    use holochain_types::dht_op::{ChainOp, DhtOp};

    let action_hash = op.action_hash();
    let Some(sah) = actions.iter().find(|sah| sah.as_hash() == action_hash) else {
        return 0;
    };
    let signed_action: LegacySignedAction = (sah.action().clone(), sah.signature().clone()).into();
    let maybe_entry: Option<Entry> = signed_action
        .data()
        .entry_hash()
        .and_then(|eh| entries.iter().find(|e| e.as_hash() == eh))
        .map(|e| e.as_content().clone());

    match ChainOp::from_type(op.get_type(), signed_action, maybe_entry) {
        Ok(chain_op) => holochain_serialized_bytes::encode(&DhtOp::from(chain_op))
            .map(|b| b.len() as u32)
            .unwrap_or(0),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_chain::SourceChainResult;
    use ::fixt::fixt;
    use ::fixt::prelude::*;
    use holo_hash::fixt::DnaHashFixturator;
    use holo_hash::fixt::{ActionHashFixturator, AgentPubKeyFixturator, EntryHashFixturator};
    use holochain_keystore::test_keystore;
    use holochain_zome_types::Entry;
    use matches::assert_matches;
    use std::collections::{BTreeSet, HashSet};

    #[tokio::test(flavor = "multi_thread")]
    async fn test_relaxed_ordering() -> SourceChainResult<()> {
        let TestCase {
            chain: chain_1,
            agent_key: alice,
            dht_store,
            keystore,
            ..
        } = TestCase::new().await;

        let chain_2 = SourceChain::new(dht_store.clone(), keystore.clone(), alice.clone()).await?;
        let chain_3 = SourceChain::new(dht_store.clone(), keystore.clone(), alice.clone()).await?;

        let action_builder = builder::CloseChain { new_target: None };
        chain_1
            .put(action_builder.clone(), None, ChainTopOrdering::Strict)
            .await?;
        chain_2
            .put(action_builder.clone(), None, ChainTopOrdering::Strict)
            .await?;
        chain_3
            .put(action_builder, None, ChainTopOrdering::Relaxed)
            .await?;

        let storage_arcs = vec![DhtArc::Empty];
        chain_1.flush(storage_arcs.clone()).await?;
        // Read the chain head from the DhtStore.
        let seq = dht_store
            .as_read()
            .chain_head_for_author(&alice)
            .await?
            .expect("chain head present after flush")
            .seq;
        assert_eq!(seq, 3);

        assert!(matches!(
            chain_2.flush(storage_arcs.clone()).await,
            Err(SourceChainError::HeadMoved(_, _, _, _))
        ));
        // Read the chain head from the DhtStore.
        let seq = dht_store
            .as_read()
            .chain_head_for_author(&alice)
            .await?
            .expect("chain head present after flush")
            .seq;
        assert_eq!(seq, 3);

        chain_3.flush(storage_arcs).await?;
        // Read the chain head from the DhtStore.
        let seq = dht_store
            .as_read()
            .chain_head_for_author(&alice)
            .await?
            .expect("chain head present after flush")
            .seq;
        assert_eq!(seq, 4);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_relaxed_ordering_with_entry() -> SourceChainResult<()> {
        let TestCase {
            chain: chain_1,
            agent_key: alice,
            dht_store,
            keystore,
            ..
        } = TestCase::new().await;

        let chain_2 = SourceChain::new(dht_store.clone(), keystore.clone(), alice.clone()).await?;
        let chain_3 = SourceChain::new(dht_store.clone(), keystore.clone(), alice.clone()).await?;

        let entry_1 = Entry::App(fixt!(AppEntryBytes));
        let eh1 = EntryHash::with_data_sync(&entry_1);
        let create = builder::Create {
            entry_type: EntryType::App(fixt!(AppEntryDef)),
            entry_hash: eh1.clone(),
        };
        let h1 = chain_1
            .put_weightless(create, Some(entry_1.clone()), ChainTopOrdering::Strict)
            .await
            .unwrap();

        let entry_err = Entry::App(fixt!(AppEntryBytes));
        let entry_hash_err = EntryHash::with_data_sync(&entry_err);
        let create = builder::Create {
            entry_type: EntryType::App(fixt!(AppEntryDef)),
            entry_hash: entry_hash_err.clone(),
        };
        chain_2
            .put_weightless(create, Some(entry_err.clone()), ChainTopOrdering::Strict)
            .await
            .unwrap();

        let entry_2 = Entry::App(fixt!(AppEntryBytes));
        let eh2 = EntryHash::with_data_sync(&entry_2);
        let create = builder::Create {
            entry_type: EntryType::App(AppEntryDef::new(
                EntryDefIndex(0),
                0.into(),
                EntryVisibility::Private,
            )),
            entry_hash: eh2.clone(),
        };
        let old_h2 = chain_3
            .put_weightless(create, Some(entry_2.clone()), ChainTopOrdering::Relaxed)
            .await
            .unwrap();

        let storage_arcs = vec![DhtArc::Empty];
        chain_1.flush(storage_arcs.clone()).await?;
        // Read the chain head from the DhtStore.
        let seq = dht_store
            .as_read()
            .chain_head_for_author(&alice)
            .await?
            .expect("chain head present after flush")
            .seq;
        assert_eq!(seq, 3);

        assert!(matches!(
            chain_2.flush(storage_arcs.clone()).await,
            Err(SourceChainError::HeadMoved(_, _, _, _))
        ));

        chain_3.flush(storage_arcs).await?;
        // Read the chain head from the DhtStore.
        let head = dht_store
            .as_read()
            .chain_head_for_author(&alice)
            .await?
            .expect("chain head present after flush");

        // not equal since action hash change due to rebasing
        assert_ne!(head.action, old_h2);
        assert_eq!(head.seq, 4);

        // The full records are read from the DhtStore. h1 is public; h2 (the
        // head) is a private entry, so the author key is passed so the store
        // attaches the author's `PrivateEntry`.
        let h1_record_entry_fetched = dht_store
            .as_read()
            .retrieve_record(&h1, Some(&alice))
            .await?
            .expect("h1 record present in store")
            .into_inner()
            .1;
        let h2_record_entry_fetched = dht_store
            .as_read()
            .retrieve_record(&head.action, Some(&alice))
            .await?
            .expect("h2 record present in store")
            .into_inner()
            .1;
        assert_eq!(RecordEntry::Present(entry_1), h1_record_entry_fetched);
        assert_eq!(RecordEntry::Present(entry_2), h2_record_entry_fetched);

        Ok(())
    }

    // The genesis agent-key `Create` is read back from the DhtStore as the
    // valid agent-key action.
    #[tokio::test(flavor = "multi_thread")]
    async fn valid_create_agent_key_action_reads_from_store() {
        let TestCase {
            chain, agent_key, ..
        } = TestCase::new().await;

        let action = chain.valid_create_agent_key_action().await.unwrap();

        // `valid_create_agent_key_action` reads from the (v2) DhtStore, so the
        // returned action is a v2 `Action`.
        // It is the agent-key `Create`: an `AgentPubKey`-typed `Create` whose
        // entry hash is the agent key.
        assert_matches!(action.data, holochain_zome_types::action::ActionData::Create(_));
        assert_eq!(action.entry_type(), Some(&EntryType::AgentPubKey));
        let agent_key_entry_hash: EntryHash = agent_key.into();
        assert_eq!(action.entry_hash(), Some(&agent_key_entry_hash));
    }

    // Test that a valid agent pub key can be deleted and that repeated deletes fail.
    #[tokio::test(flavor = "multi_thread")]
    async fn delete_valid_agent_pub_key() {
        let TestCase { chain, .. } = TestCase::new().await;

        let result = chain.delete_valid_agent_pub_key().await;
        assert!(result.is_ok());
        chain.flush(vec![DhtArc::Empty]).await.unwrap();

        // Valid agent pub key has been deleted. Repeating the operation should fail now as no valid
        // pub key can be found.
        let result = chain.delete_valid_agent_pub_key().await.unwrap_err();
        assert_matches!(result, SourceChainError::InvalidAgentKey(invalid_key, cell_id) if invalid_key == *chain.author && cell_id == *chain.cell_id());
    }

    // An `Update` targeting the agent-key entry invalidates the key, just like
    // a `Delete` does, so `valid_create_agent_key_action` returns
    // `InvalidAgentKey`.
    #[tokio::test(flavor = "multi_thread")]
    async fn updated_agent_key_is_invalid() {
        let TestCase {
            chain, agent_key, ..
        } = TestCase::new().await;

        // Valid before any modification.
        let create = chain.valid_create_agent_key_action().await.unwrap();
        let agent_key_entry_hash: EntryHash = agent_key.clone().into();

        // Author an `Update` whose original entry is the agent-key entry. This
        // populates the `UpdatedRecord` index (keyed on `original_entry_hash`)
        // for the agent key.
        let action_builder = builder::Update {
            entry_type: EntryType::AgentPubKey,
            entry_hash: agent_key_entry_hash.clone(),
            original_action_address: create.to_hash(),
            original_entry_address: agent_key_entry_hash,
        };
        chain
            .put_weightless(
                action_builder,
                Some(Entry::Agent(agent_key.clone())),
                ChainTopOrdering::default(),
            )
            .await
            .unwrap();
        chain.flush(vec![DhtArc::Empty]).await.unwrap();

        let result = chain.valid_create_agent_key_action().await.unwrap_err();
        assert_matches!(
            result,
            SourceChainError::InvalidAgentKey(invalid_key, _) if invalid_key == agent_key
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_cap_grant() -> SourceChainResult<()> {
        let TestCase {
            chain,
            agent_key: alice,
            dht_store,
            keystore,
            ..
        } = TestCase::new().await;

        let secret = Some(CapSecretFixturator::new(Unpredictable).next().unwrap());
        // create transferable cap grant
        #[allow(clippy::unnecessary_literal_unwrap)] // must be this type
        let secret_access = CapAccess::from(secret.unwrap());

        // @todo curry
        let _curry = CurryPayloadsFixturator::new(Empty).next().unwrap();
        let function: GrantedFunction = ("foo".into(), "bar".into());
        let mut fns = HashSet::new();
        fns.insert(function.clone());
        let functions = GrantedFunctions::Listed(fns);
        let grant = ZomeCallCapGrant::new("tag".into(), secret_access.clone(), functions.clone());

        let bob = keystore.new_sign_keypair_random().await.unwrap();
        let carol = keystore.new_sign_keypair_random().await.unwrap();

        // alice as chain author always has a valid cap grant; provided secrets
        // are ignored
        assert_eq!(
            chain
                .valid_cap_grant(function.clone(), alice.clone(), secret)
                .await?,
            Some(CapGrant::ChainAuthor(alice.clone())),
        );

        // bob should not get a cap grant as the secret hasn't been committed yet
        assert_eq!(
            chain
                .valid_cap_grant(function.clone(), bob.clone(), secret)
                .await?,
            None
        );

        let storage_arcs = vec![DhtArc::Empty];

        // write cap grant to alice's source chain
        let (original_action_address, original_entry_address) = {
            let (entry, entry_hash) =
                EntryHashed::from_content_sync(Entry::CapGrant(grant.clone())).into_inner();
            let action_builder = builder::Create {
                entry_type: EntryType::CapGrant,
                entry_hash: entry_hash.clone(),
            };
            let action = chain
                .put_weightless(action_builder, Some(entry), ChainTopOrdering::default())
                .await?;

            chain.flush(storage_arcs.clone()).await.unwrap();
            (action, entry_hash)
        };

        // alice should find her own authorship with higher priority than the
        // committed grant even if she passes in the secret
        assert_eq!(
            chain
                .valid_cap_grant(function.clone(), alice.clone(), secret)
                .await?,
            Some(CapGrant::ChainAuthor(alice.clone())),
        );

        // bob and carol (and everyone else) should be authorized with transferable cap grant
        // when passing in the secret
        assert_eq!(
            chain
                .valid_cap_grant(function.clone(), bob.clone(), secret)
                .await?,
            Some(grant.clone().into())
        );
        assert_eq!(
            chain
                .valid_cap_grant(function.clone(), carol.clone(), secret)
                .await?,
            Some(grant.clone().into())
        );
        // bob should not be authorized for other zomes/functions than the ones granted
        assert_eq!(
            chain
                .valid_cap_grant(("boo".into(), "far".into()), bob.clone(), secret)
                .await?,
            None
        );

        // convert transferable cap grant to assigned with bob as assignee
        let mut assignees = BTreeSet::new();
        assignees.insert(bob.clone());
        let updated_secret = Some(CapSecretFixturator::new(Unpredictable).next().unwrap());
        #[allow(clippy::unnecessary_literal_unwrap)] // must be this type
        let updated_access = CapAccess::from((updated_secret.unwrap(), assignees));
        let updated_grant = ZomeCallCapGrant::new("tag".into(), updated_access.clone(), functions);

        // commit grant update to alice's source chain
        let (updated_action_hash, updated_entry_hash) = {
            let chain =
                SourceChain::new(dht_store.clone(), keystore.clone(), alice.clone()).await?;
            let (entry, entry_hash) =
                EntryHashed::from_content_sync(Entry::CapGrant(updated_grant.clone())).into_inner();
            let action_builder = builder::Update {
                entry_type: EntryType::CapGrant,
                entry_hash: entry_hash.clone(),
                original_action_address,
                original_entry_address,
            };
            let action = chain
                .put_weightless(action_builder, Some(entry), ChainTopOrdering::default())
                .await?;
            chain.flush(storage_arcs.clone()).await.unwrap();

            (action, entry_hash)
        };

        // alice as chain owner should be unaffected by updates
        // chain author grant should always be returned
        assert_eq!(
            chain
                .valid_cap_grant(function.clone(), alice.clone(), secret)
                .await?,
            Some(CapGrant::ChainAuthor(alice.clone())),
        );
        assert_eq!(
            chain
                .valid_cap_grant(function.clone(), alice.clone(), updated_secret)
                .await?,
            Some(CapGrant::ChainAuthor(alice.clone())),
        );

        // bob must not get a valid cap grant with the initial cap secret,
        // as it is invalidated by the cap grant update
        assert_eq!(
            chain
                .valid_cap_grant(function.clone(), bob.clone(), secret)
                .await?,
            None
        );
        // when bob provides the updated secret, validation should succeed,
        // bob being an assignee
        assert_eq!(
            chain
                .valid_cap_grant(function.clone(), bob.clone(), updated_secret)
                .await?,
            Some(updated_grant.clone().into())
        );

        // carol must not get a valid cap grant with either the original secret (because it was replaced)
        // or the updated secret (because she is not an assignee)
        assert_eq!(
            chain
                .valid_cap_grant(function.clone(), carol.clone(), secret)
                .await?,
            None
        );
        assert_eq!(
            chain
                .valid_cap_grant(function.clone(), carol.clone(), updated_secret)
                .await?,
            None
        );

        // Two source chains of the same DNA on the same conductor share DB tables.
        // That could lead to cap grants looked up by their secret alone being
        // returned for any agent on the conductor,in this case for alice trying
        // to access carol's chain
        {
            let extra_dht_store = crate::test_utils::test_dht_store(fake_dna_hash(1)).await;
            genesis(
                extra_dht_store.clone(),
                keystore.clone(),
                fake_dna_hash(1),
                carol.clone(),
                None,
            )
            .await
            .unwrap();
            let carol_chain = SourceChain::new(extra_dht_store, keystore.clone(), carol.clone())
                .await
                .unwrap();
            let maybe_cap_grant = carol_chain
                .valid_cap_grant(("".into(), "".into()), alice.clone(), secret)
                .await
                .unwrap();
            assert_eq!(maybe_cap_grant, None);
        }

        // delete updated cap grant
        {
            let chain =
                SourceChain::new(dht_store.clone(), keystore.clone(), alice.clone()).await?;
            let action_builder = builder::Delete {
                deletes_address: updated_action_hash,
                deletes_entry_address: updated_entry_hash,
            };
            chain
                .put_weightless(action_builder, None, ChainTopOrdering::default())
                .await?;
            chain.flush(storage_arcs.clone()).await.unwrap();
        }

        // alice should get her author cap grant as always, independent of
        // any provided secret
        assert_eq!(
            chain
                .valid_cap_grant(function.clone(), alice.clone(), secret)
                .await?,
            Some(CapGrant::ChainAuthor(alice.clone())),
        );
        assert_eq!(
            chain
                .valid_cap_grant(function.clone(), alice.clone(), updated_secret)
                .await?,
            Some(CapGrant::ChainAuthor(alice.clone())),
        );

        // bob should not get a cap grant for any secret anymore
        assert_eq!(
            chain
                .valid_cap_grant(function.clone(), bob.clone(), secret)
                .await?,
            None
        );
        assert_eq!(
            chain
                .valid_cap_grant(function.clone(), bob.clone(), updated_secret)
                .await?,
            None
        );

        // create an unrestricted cap grant in alice's chain
        let unrestricted_grant = ZomeCallCapGrant::new(
            "unrestricted".into(),
            CapAccess::Unrestricted,
            GrantedFunctions::All,
        );
        let (original_action_address, original_entry_address) = {
            let chain =
                SourceChain::new(dht_store.clone(), keystore.clone(), alice.clone()).await?;
            let (entry, entry_hash) =
                EntryHashed::from_content_sync(Entry::CapGrant(unrestricted_grant.clone()))
                    .into_inner();
            let action_builder = builder::Create {
                entry_type: EntryType::CapGrant,
                entry_hash: entry_hash.clone(),
            };
            let action = chain
                .put_weightless(action_builder, Some(entry), ChainTopOrdering::default())
                .await?;
            chain.flush(storage_arcs.clone()).await.unwrap();
            (action, entry_hash)
        };

        // bob should get a cap grant for any zome and function
        let granted_function: GrantedFunction = ("zome".into(), "fn".into());
        assert_eq!(
            chain
                .valid_cap_grant(granted_function.clone(), bob.clone(), None)
                .await?,
            Some(unrestricted_grant.clone().into())
        );
        // carol should get a cap grant now too
        assert_eq!(
            chain
                .valid_cap_grant(granted_function.clone(), carol.clone(), None)
                .await?,
            Some(unrestricted_grant.clone().into())
        );
        // but not for bob's chain
        //
        // Two source chains of the same DNA on the same conductor share DB tables.
        // That could lead to cap grants looked up by being unrestricted alone
        // being returned for any agent on the conductor.
        // In this case carol should not get an unrestricted cap grant for
        // bob's chain.
        {
            {
                let extra_dht_store = crate::test_utils::test_dht_store(fake_dna_hash(1)).await;
                genesis(
                    extra_dht_store.clone(),
                    keystore.clone(),
                    fake_dna_hash(1),
                    bob.clone(),
                    None,
                )
                .await
                .unwrap();
                let bob_chain = SourceChain::new(extra_dht_store, keystore.clone(), bob.clone())
                    .await
                    .unwrap();
                let maybe_cap_grant = bob_chain
                    .valid_cap_grant(("".into(), "".into()), carol.clone(), None)
                    .await
                    .unwrap();
                assert_eq!(maybe_cap_grant, None);
            }
        }

        // delete unrestricted cap grant
        {
            let chain =
                SourceChain::new(dht_store.clone(), keystore.clone(), alice.clone()).await?;
            let action_builder = builder::Delete {
                deletes_address: original_action_address,
                deletes_entry_address: original_entry_address,
            };
            chain
                .put_weightless(action_builder, None, ChainTopOrdering::default())
                .await?;
            chain.flush(storage_arcs.clone()).await.unwrap();
        }

        // bob must not get unrestricted cap grant any longer
        assert_eq!(
            chain
                .valid_cap_grant(granted_function.clone(), bob.clone(), None)
                .await?,
            None
        );

        // Create two unrestricted cap grants in alice's chain to make sure
        // that all of them are considered when checking grant validity
        // instead of only the first cap grant found.

        // first unrestricted cap grant with irrelevant zome and fn
        let some_zome_name: ZomeName = "some_zome".into();
        let some_fn_name: FunctionName = "some_fn".into();
        let mut granted_fns = HashSet::new();
        granted_fns.insert((some_zome_name.clone(), some_fn_name.clone()));
        let first_unrestricted_grant = ZomeCallCapGrant::new(
            "unrestricted_1".into(),
            CapAccess::Unrestricted,
            GrantedFunctions::Listed(granted_fns),
        );

        // second unrestricted cap grant with the actually granted zome and fn
        let granted_zome_name: ZomeName = "granted_zome".into();
        let granted_fn_name: FunctionName = "granted_fn".into();
        let mut granted_fns = HashSet::new();
        granted_fns.insert((granted_zome_name.clone(), granted_fn_name.clone()));
        let second_unrestricted_grant = ZomeCallCapGrant::new(
            "unrestricted_2".into(),
            CapAccess::Unrestricted,
            GrantedFunctions::Listed(granted_fns),
        );

        {
            let chain =
                SourceChain::new(dht_store.clone(), keystore.clone(), alice.clone()).await?;

            // commit first grant to alice's chain
            let (entry, entry_hash) =
                EntryHashed::from_content_sync(Entry::CapGrant(first_unrestricted_grant.clone()))
                    .into_inner();
            let action_builder = builder::Create {
                entry_type: EntryType::CapGrant,
                entry_hash: entry_hash.clone(),
            };
            let _ = chain
                .put_weightless(action_builder, Some(entry), ChainTopOrdering::default())
                .await?;

            // commit second grant to alice's chain
            let (entry, entry_hash) =
                EntryHashed::from_content_sync(Entry::CapGrant(second_unrestricted_grant.clone()))
                    .into_inner();
            let action_builder = builder::Create {
                entry_type: EntryType::CapGrant,
                entry_hash: entry_hash.clone(),
            };
            let _ = chain
                .put_weightless(action_builder, Some(entry), ChainTopOrdering::default())
                .await?;

            chain.flush(storage_arcs).await.unwrap();
        }

        let actual_cap_grant = chain
            .valid_cap_grant((granted_zome_name, granted_fn_name), bob, None)
            .await
            .unwrap();
        assert_eq!(actual_cap_grant, Some(second_unrestricted_grant.into()));

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn source_chain_buffer_iter_back() -> SourceChainResult<()> {
        holochain_trace::test_run();
        let keystore = test_keystore();
        let dna_hash = fixt!(DnaHash);
        let dht_store = crate::test_utils::test_dht_store(dna_hash.clone()).await;

        let author = Arc::new(keystore.new_sign_keypair_random().await.unwrap());

        genesis(
            dht_store.clone(),
            keystore.clone(),
            dna_hash,
            (*author).clone(),
            None,
        )
        .await
        .unwrap();

        let source_chain = SourceChain::new(dht_store.clone(), keystore.clone(), (*author).clone())
            .await
            .unwrap();
        let entry = Entry::App(fixt!(AppEntryBytes));
        let create = builder::Create {
            entry_type: EntryType::App(fixt!(AppEntryDef)),
            entry_hash: EntryHash::with_data_sync(&entry),
        };
        let h1 = source_chain
            .put_weightless(create, Some(entry), ChainTopOrdering::default())
            .await
            .unwrap();
        let entry = Entry::App(fixt!(AppEntryBytes));
        let create = builder::Create {
            entry_type: EntryType::App(fixt!(AppEntryDef)),
            entry_hash: EntryHash::with_data_sync(&entry),
        };
        let h2 = source_chain
            .put_weightless(create, Some(entry), ChainTopOrdering::default())
            .await
            .unwrap();
        source_chain.flush(vec![DhtArc::Empty]).await.unwrap();

        // The head and full records are read from the DhtStore.
        let head = dht_store
            .as_read()
            .chain_head_for_author(author.as_ref())
            .await?
            .expect("chain head present after flush");
        assert_eq!(head.action, h2);

        let h1_record_fetched = dht_store
            .as_read()
            .retrieve_record(&h1, Some(author.as_ref()))
            .await?
            .expect("h1 record present in store");
        let h2_record_fetched = dht_store
            .as_read()
            .retrieve_record(&h2, Some(author.as_ref()))
            .await?
            .expect("h2 record present in store");
        assert_eq!(h1, *h1_record_fetched.action_address());
        assert_eq!(h2, *h2_record_fetched.action_address());

        // check that you can iterate on the chain
        let source_chain = SourceChain::new(dht_store.clone(), keystore.clone(), (*author).clone())
            .await
            .unwrap();
        let res = source_chain.query(QueryFilter::new()).await.unwrap();
        assert_eq!(res.len(), 5);
        assert_eq!(*res[3].action_address(), h1);
        assert_eq!(*res[4].action_address(), h2);

        Ok(())
    }

    /// After `genesis`, the store reports the author has done genesis, the
    /// chain head is the seq-2 AgentId `Create`, and the head record is
    /// retrievable from the store.
    #[tokio::test(flavor = "multi_thread")]
    async fn genesis_writes_to_merged_store() -> SourceChainResult<()> {
        holochain_trace::test_run();
        let keystore = test_keystore();
        let dna_hash = fixt!(DnaHash);
        let dht_store = crate::test_utils::test_dht_store(dna_hash.clone()).await;
        let author = keystore.new_sign_keypair_random().await.unwrap();

        genesis(
            dht_store.clone(),
            keystore.clone(),
            dna_hash,
            author.clone(),
            None,
        )
        .await
        .unwrap();

        let store = dht_store.as_read();

        // `has_genesis` requires all three genesis actions to be present.
        assert!(store.has_genesis(&author).await?);

        // The chain head is the seq-2 AgentId `Create`.
        let head = store
            .chain_head_for_author(&author)
            .await?
            .expect("chain head present after genesis");
        assert_eq!(head.seq, 2);

        let head_record = store
            .retrieve_record(&head.action, Some(&author))
            .await?
            .expect("head record present in store");
        // `retrieve_record` reads from the (v2) DhtStore, so the record's
        // action is a v2 `Action`.
        assert_eq!(head_record.action().action_seq(), 2);
        assert!(matches!(
            head_record.action().data,
            holochain_zome_types::action::ActionData::Create(_)
        ));

        Ok(())
    }

    /// Verify that `DhtStore::dump_source_chain` returns records in seq order,
    /// resolves private-entry records' entry data from `PrivateEntry`, and
    /// reports the correct published-op count.
    #[tokio::test(flavor = "multi_thread")]
    async fn dump_state_from_store() -> SourceChainResult<()> {
        let TestCase {
            chain,
            agent_key,
            dht_store,
            ..
        } = TestCase::new().await;

        // Add a private-entry action (seq 3) on top of the genesis 3-action chain.
        let private_entry = Entry::App(fixt!(AppEntryBytes));
        let private_entry_hash = EntryHash::with_data_sync(&private_entry);
        let create = builder::Create {
            entry_type: EntryType::App(AppEntryDef::new(
                0.into(),
                0.into(),
                EntryVisibility::Private,
            )),
            entry_hash: private_entry_hash.clone(),
        };
        chain
            .put_weightless(
                create,
                Some(private_entry.clone()),
                ChainTopOrdering::default(),
            )
            .await?;
        chain.flush(vec![DhtArc::Empty]).await?;

        let dump = dht_store.as_read().dump_source_chain(&agent_key).await?;

        // Four records: Dna(0), AgentValidationPkg(1), Create/AgentId(2), Create/Private(3).
        assert_eq!(
            dump.records.len(),
            4,
            "expected 4 records after genesis + 1"
        );

        // Verify seq order.
        for (i, rec) in dump.records.iter().enumerate() {
            assert_eq!(rec.action.action_seq(), i as u32, "record {i} out of order");
        }

        // The private-entry record (seq 3) must expose its entry.
        let private_rec = &dump.records[3];
        assert_eq!(
            private_rec.entry.as_ref(),
            Some(&private_entry),
            "private-entry record must include the entry"
        );

        // No publishing has occurred — published_ops_count must be 0.
        assert_eq!(
            dump.published_ops_count, 0,
            "no ops have been published yet"
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn source_chain_buffer_dump_entries_json() -> SourceChainResult<()> {
        let TestCase {
            chain: _,
            agent_key,
            dht_store,
            ..
        } = TestCase::new().await;

        let json = dump_state(&dht_store.as_read(), agent_key.clone()).await?;
        let json = serde_json::to_string_pretty(&json)?;
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["records"][0]["action"]["type"], "Dna");
        assert_eq!(parsed["records"][0]["entry"], serde_json::Value::Null);

        assert_eq!(parsed["records"][2]["action"]["type"], "Create");
        assert_eq!(parsed["records"][2]["action"]["entry_type"], "AgentPubKey");
        assert_eq!(parsed["records"][2]["entry"]["entry_type"], "Agent");
        assert_ne!(
            parsed["records"][2]["entry"]["entry"],
            serde_json::Value::Null
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn source_chain_query() {
        let TestCase {
            chain,
            agent_key: alice,
            keystore,
            ..
        } = TestCase::new().await;

        let app_entry_type = EntryType::App(AppEntryDef {
            zome_index: 0.into(),
            entry_index: 0.into(),
            visibility: EntryVisibility::Public,
        });

        let chain_top = chain.chain_head_nonempty().unwrap();

        // Add an app entry to the chain
        let create_action = {
            let entry = Entry::App(fixt!(AppEntryBytes));
            let entry_hashed = EntryHashed::from_content_sync(entry);

            let action = Action::Create(Create {
                author: alice.clone(),
                timestamp: Timestamp::now(),
                action_seq: chain_top.seq + 1,
                prev_action: chain_top.action.as_hash().clone(),
                entry_type: app_entry_type.clone(),
                entry_hash: entry_hashed.hash.clone(),
                weight: EntryRateWeight::default(),
            });
            let v2_action = holochain_zome_types::dht_v2::from_legacy_action(&action);
            let sig = alice.sign(&keystore, &v2_action).await.unwrap();
            let signed_action =
                SignedActionHashed::with_presigned(ActionHashed::from_content_sync(action.clone()), sig);

            chain
                .scratch()
                .apply(move |scratch| {
                    scratch.add_action(signed_action, ChainTopOrdering::Strict);
                    scratch.add_entry(entry_hashed, ChainTopOrdering::Strict);
                })
                .unwrap();
            chain.flush(vec![DhtArc::Empty]).await.unwrap();

            action
        };

        // Add an app entry to the scratch space
        {
            let chain_top = chain.chain_head_nonempty().unwrap();

            let entry = Entry::App(fixt!(AppEntryBytes));
            let entry_hashed = EntryHashed::from_content_sync(entry);

            let action = Action::Update(Update {
                author: alice.clone(),
                timestamp: Timestamp::now(),
                action_seq: chain_top.seq + 2,
                prev_action: create_action.to_hash(),
                original_action_address: create_action.to_hash(),
                original_entry_address: create_action.entry_hash().unwrap().clone(),
                entry_type: app_entry_type.clone(),
                entry_hash: entry_hashed.hash.clone(),
                weight: EntryRateWeight::default(),
            });
            let v2_action = holochain_zome_types::dht_v2::from_legacy_action(&action);
            let sig = alice.sign(&keystore, &v2_action).await.unwrap();
            let signed_action =
                SignedActionHashed::with_presigned(ActionHashed::from_content_sync(action), sig);

            chain
                .scratch()
                .apply(move |scratch| {
                    scratch.add_action(signed_action, ChainTopOrdering::Strict);
                    scratch.add_entry(entry_hashed, ChainTopOrdering::Strict);
                })
                .unwrap();
        }

        let records = chain.query(ChainQueryFilter::default()).await.unwrap();

        // All the range queries which should return a full set of records
        let full_ranges = [
            ChainQueryFilterRange::Unbounded,
            ChainQueryFilterRange::ActionSeqRange(0, 4),
            ChainQueryFilterRange::ActionHashRange(
                records[0].action_address().clone(),
                records[4].action_address().clone(),
            ),
            ChainQueryFilterRange::ActionHashTerminated(records[4].action_address().clone(), 4),
        ];

        // A variety of combinations of query parameters
        let cases = [
            ((None, None, vec![], false), 5),
            ((None, None, vec![], true), 5),
            ((Some(vec![ActionType::Dna]), None, vec![], false), 1),
            ((None, Some(vec![EntryType::AgentPubKey]), vec![], false), 1),
            ((None, Some(vec![EntryType::AgentPubKey]), vec![], true), 1),
            ((Some(vec![ActionType::Create]), None, vec![], false), 2),
            ((Some(vec![ActionType::Create]), None, vec![], true), 2),
            (
                (
                    Some(vec![ActionType::Create]),
                    Some(vec![EntryType::AgentPubKey]),
                    vec![],
                    false,
                ),
                1,
            ),
            (
                (
                    Some(vec![ActionType::Create]),
                    Some(vec![EntryType::AgentPubKey]),
                    vec![records[2].action().entry_hash().unwrap().clone()],
                    true,
                ),
                1,
            ),
            (
                (
                    Some(vec![ActionType::Create, ActionType::Dna]),
                    None,
                    vec![],
                    true,
                ),
                3,
            ),
            (
                (
                    None,
                    Some(vec![EntryType::AgentPubKey, app_entry_type]),
                    vec![],
                    true,
                ),
                3,
            ),
        ];

        // Test all permutations of cases defined with all full range queries,
        // and both boolean values of `include_entries`.
        for ((action_type, entry_type, entry_hashes, include_entries), num_expected) in cases {
            let entry_hashes = if entry_hashes.is_empty() {
                None
            } else {
                Some(entry_hashes.into_iter().collect())
            };
            for sequence_range in full_ranges.clone() {
                let query = ChainQueryFilter {
                    sequence_range: sequence_range.clone(),
                    action_type: action_type.clone(),
                    entry_type: entry_type.clone(),
                    entry_hashes: entry_hashes.clone(),
                    include_entries,
                    order_descending: false,
                };

                let queried = chain.query(query.clone()).await.unwrap();
                let actual = queried.len();
                assert!(queried.iter().all(|e| e.action().author() == &alice));
                assert_eq!(
                    num_expected, actual,
                    "Expected {num_expected} items but got {actual} with filter {query:?}"
                );
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn source_chain_query_private_entry_redacted_under_public_only() {
        let TestCase {
            mut chain,
            agent_key: alice,
            keystore,
            ..
        } = TestCase::new().await;

        let private_entry_type = EntryType::App(AppEntryDef {
            zome_index: 0.into(),
            entry_index: 0.into(),
            visibility: EntryVisibility::Private,
        });
        let public_entry_type = EntryType::App(AppEntryDef {
            zome_index: 0.into(),
            entry_index: 1.into(),
            visibility: EntryVisibility::Public,
        });

        // Commit a Create with a PRIVATE entry to the DhtStore via flush.
        let chain_top = chain.chain_head_nonempty().unwrap();
        let private_entry_hashed = EntryHashed::from_content_sync(Entry::App(fixt!(AppEntryBytes)));
        let private_create = Action::Create(Create {
            author: alice.clone(),
            timestamp: Timestamp::now(),
            action_seq: chain_top.seq + 1,
            prev_action: chain_top.action.as_hash().clone(),
            entry_type: private_entry_type.clone(),
            entry_hash: private_entry_hashed.hash.clone(),
            weight: EntryRateWeight::default(),
        });
        let v2_private_create = holochain_zome_types::dht_v2::from_legacy_action(&private_create);
        let sig = alice.sign(&keystore, &v2_private_create).await.unwrap();
        let private_sah =
            SignedActionHashed::with_presigned(ActionHashed::from_content_sync(private_create), sig);
        let private_action_hash = private_sah.as_hash().clone();
        chain
            .scratch()
            .apply({
                let private_sah = private_sah.clone();
                move |scratch| {
                    scratch.add_action(private_sah, ChainTopOrdering::Strict);
                    scratch.add_entry(private_entry_hashed, ChainTopOrdering::Strict);
                }
            })
            .unwrap();
        chain.flush(vec![DhtArc::Empty]).await.unwrap();

        // Add an uncommitted public Create to the scratch.
        let chain_top = chain.chain_head_nonempty().unwrap();
        let public_entry_hashed = EntryHashed::from_content_sync(Entry::App(fixt!(AppEntryBytes)));
        let public_create = Action::Create(Create {
            author: alice.clone(),
            timestamp: Timestamp::now(),
            action_seq: chain_top.seq + 1,
            prev_action: chain_top.action.as_hash().clone(),
            entry_type: public_entry_type.clone(),
            entry_hash: public_entry_hashed.hash.clone(),
            weight: EntryRateWeight::default(),
        });
        let v2_public_create = holochain_zome_types::dht_v2::from_legacy_action(&public_create);
        let sig = alice.sign(&keystore, &v2_public_create).await.unwrap();
        let public_sah =
            SignedActionHashed::with_presigned(ActionHashed::from_content_sync(public_create), sig);
        let scratch_action_hash = public_sah.as_hash().clone();
        chain
            .scratch()
            .apply({
                let public_sah = public_sah.clone();
                move |scratch| {
                    scratch.add_action(public_sah, ChainTopOrdering::Strict);
                    scratch.add_entry(public_entry_hashed, ChainTopOrdering::Strict);
                }
            })
            .unwrap();

        // With full visibility, the committed private entry is present and the
        // uncommitted scratch record is visible.
        let q = ChainQueryFilter::default().include_entries(true);
        let records = chain.query(q.clone()).await.unwrap();
        let committed_private = records
            .iter()
            .find(|r| r.action_address() == &private_action_hash)
            .expect("committed private record present");
        assert!(
            matches!(committed_private.entry(), RecordEntry::Present(_)),
            "private entry should be present without public_only"
        );
        assert!(
            records
                .iter()
                .any(|r| r.action_address() == &scratch_action_hash),
            "uncommitted scratch record should be visible"
        );

        // With public_only set, the committed private entry is redacted (the
        // action remains) but the scratch record — this agent's own data — still
        // carries its entry.
        chain.public_only();
        let records = chain.query(q).await.unwrap();
        let committed_private = records
            .iter()
            .find(|r| r.action_address() == &private_action_hash)
            .expect("committed private action still present under public_only");
        assert!(
            matches!(committed_private.entry(), RecordEntry::Hidden),
            "private entry should be redacted (Hidden) under public_only, got {:?}",
            committed_private.entry()
        );
        let scratch_record = records
            .iter()
            .find(|r| r.action_address() == &scratch_action_hash)
            .expect("scratch record still visible under public_only");
        assert!(
            matches!(scratch_record.entry(), RecordEntry::Present(_)),
            "scratch entry (own data) should remain present under public_only"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn source_chain_query_ordering() {
        let TestCase { chain, .. } = TestCase::new().await;

        let asc = chain.query(ChainQueryFilter::default()).await.unwrap();
        let desc = chain
            .query(ChainQueryFilter::default().descending())
            .await
            .unwrap();

        assert_eq!(asc.len(), 3);
        assert_ne!(asc, desc);

        let mut desc_sorted = desc;
        desc_sorted.sort_by_key(|r| r.action().action_seq());
        assert_eq!(asc, desc_sorted);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn init_zomes_complete() {
        let TestCase { chain, .. } = TestCase::new().await;

        // zomes initialized should be false after genesis
        let zomes_initialized = chain.zomes_initialized().await.unwrap();
        assert!(!zomes_initialized);

        // insert init marker into source chain
        let result = chain
            .put(
                builder::InitZomesComplete {},
                None,
                ChainTopOrdering::Strict,
            )
            .await;
        assert!(result.is_ok());

        chain.flush(vec![DhtArc::Empty]).await.unwrap();

        // zomes initialized should be true after init zomes has run
        let zomes_initialized = chain.zomes_initialized().await.unwrap();
        assert!(zomes_initialized);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn flush_writes_warrants_to_dht_store() {
        let TestCase {
            chain,
            agent_key,
            dht_store,
            keystore,
            ..
        } = TestCase::new().await;
        let warrantee = fixt!(AgentPubKey);

        // The warrant is authored by `agent_key` (the warrant issuer). Read it
        // back via the DhtStore (limbo + integrated).
        let actual_warrants = dht_store
            .as_read()
            .warrants_by_author(agent_key.clone())
            .await
            .unwrap();
        assert_eq!(actual_warrants.len(), 0);

        // Create a warrant
        let signed_warrant = create_signed_warrant(&agent_key, &warrantee, &keystore).await;
        // Add warrant to scratch
        chain
            .scratch
            .apply(|scratch| {
                scratch.add_warrant(signed_warrant.clone());
            })
            .unwrap();

        // Flush should write warrants to the DHT store
        let (actions, warrant_count) = chain.flush(vec![]).await.unwrap();
        assert!(actions.is_empty());
        assert_eq!(warrant_count, 1);

        // Check the DHT store
        let actual_warrants = dht_store
            .as_read()
            .warrants_by_author(agent_key.clone())
            .await
            .unwrap();
        assert_eq!(actual_warrants, vec![WarrantOp::from(signed_warrant)]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn duplicate_warrants_are_not_inserted_during_flush() {
        holochain_trace::test_run();
        let TestCase {
            chain,
            agent_key,
            dht_store,
            keystore,
            ..
        } = TestCase::new().await;
        let warrantee = fixt!(AgentPubKey);

        // Create a warrant
        let signed_warrant = create_signed_warrant(&agent_key, &warrantee, &keystore).await;
        // Add warrant to scratch
        chain
            .scratch
            .apply(|scratch| {
                scratch.add_warrant(signed_warrant.clone());
            })
            .unwrap();

        // Flush should write warrant to the DHT store
        let (actions, warrant_count) = chain.flush(vec![]).await.unwrap();
        assert!(actions.is_empty());
        assert_eq!(warrant_count, 1);

        // Check the DHT store
        let actual_warrants = dht_store
            .as_read()
            .warrants_by_author(agent_key.clone())
            .await
            .unwrap();
        assert_eq!(
            actual_warrants,
            vec![WarrantOp::from(signed_warrant.clone())]
        );

        // Add same warrant to scratch again
        chain
            .scratch
            .apply(|scratch| {
                scratch.add_warrant(signed_warrant.clone());
            })
            .unwrap();

        // Flush should not write duplicate warrant to the DHT store
        let (actions, warrant_count) = chain.flush(vec![]).await.unwrap();
        assert!(actions.is_empty());
        assert_eq!(warrant_count, 1); // rejected inserts are not reported by the insertion method, so this will indicate 1

        // Check the DHT store again
        let actual_warrants = dht_store
            .as_read()
            .warrants_by_author(agent_key.clone())
            .await
            .unwrap();
        assert_eq!(actual_warrants, vec![WarrantOp::from(signed_warrant)]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn counterfeit_warrants_are_not_inserted_during_flush() {
        let TestCase {
            chain,
            agent_key,
            dht_store,
            ..
        } = TestCase::new().await;
        let warrantee = fixt!(AgentPubKey);

        // Create a counterfeit warrant
        let warrant = Warrant::new(
            WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
                action_author: warrantee.clone(),
                action: (fixt!(ActionHash), fixt!(Signature)),
                chain_op_type: ChainOpType::RegisterAgentActivity,
                reason: "invalid chain op".into(),
            }),
            agent_key.clone(),
            Timestamp::now(),
            warrantee.clone(),
        );
        let signed_warrant = SignedWarrant::new(warrant, fixt!(Signature));
        // Add warrant to scratch
        chain
            .scratch
            .apply(|scratch| {
                scratch.add_warrant(signed_warrant.clone());
            })
            .unwrap();

        // Flush should not write warrant to DHT database
        let (actions, warrant_count) = chain.flush(vec![]).await.unwrap();
        assert!(actions.is_empty());
        assert_eq!(warrant_count, 0);

        // Check the DHT store — the counterfeit warrant must not be present.
        let actual_warrants = dht_store
            .as_read()
            .warrants_by_author(agent_key.clone())
            .await
            .unwrap();
        assert!(actual_warrants.is_empty());
    }

    /// Flush of a countersigning op writes `withhold_publish = 1` to `ChainOpPublish`
    /// in the new DHT schema.
    #[tokio::test(flavor = "multi_thread")]
    async fn flush_countersigning_op_sets_withhold_publish() {
        use holochain_zome_types::countersigning::{
            CounterSigningAgentState, CounterSigningSessionData, CounterSigningSessionTimes,
            PreflightRequest,
        };
        use std::time::Duration;

        let TestCase {
            chain,
            agent_key: alice,
            dht_store,
            keystore,
            ..
        } = TestCase::new().await;

        // Second signing agent — exists only as a key; no local chain needed.
        let bob = keystore.new_sign_keypair_random().await.unwrap();

        // Build a preflight request for alice (index 0) and bob (index 1).
        let app_entry_hash = fixt!(EntryHash);
        let app_entry_type = EntryType::App(AppEntryDef::new(
            EntryDefIndex(0),
            0.into(),
            EntryVisibility::Public,
        ));
        let start = Timestamp::now();
        let end = (start + Duration::from_secs(60)).unwrap();
        let session_times = CounterSigningSessionTimes::try_new(start, end).unwrap();
        let preflight_request = PreflightRequest::try_new(
            app_entry_hash,
            vec![(alice.clone(), vec![]), (bob.clone(), vec![])],
            vec![],
            0,
            false,
            session_times,
            ActionBase::Create(CreateBase::new(app_entry_type.clone())),
            PreflightBytes(vec![]),
        )
        .unwrap();

        // Alice accepts — this locks her chain and returns her agent state.
        let alice_agent_state = chain
            .accept_countersigning_preflight_request(preflight_request.clone(), 0)
            .await
            .unwrap();

        // Build a fake Bob agent state (index 1). The test only cares that
        // the flush path sets `withhold_publish`; full signature verification
        // is not exercised here.
        let bob_agent_state = CounterSigningAgentState::new(1, fixt!(ActionHash), 2);

        let session_data = CounterSigningSessionData::try_new(
            preflight_request,
            vec![
                (alice_agent_state, fixt!(Signature)),
                (bob_agent_state, fixt!(Signature)),
            ],
            vec![],
        )
        .unwrap();

        let entry = Entry::CounterSign(Box::new(session_data), fixt!(AppEntryBytes));
        chain
            .put_countersigned(entry, ChainTopOrdering::Strict, EntryRateWeight::default())
            .await
            .unwrap();

        chain.flush(vec![DhtArc::Empty]).await.unwrap();

        // Assert that at least one ChainOpPublish row carries withhold_publish = 1.
        let withheld_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM ChainOpPublish WHERE withhold_publish = 1")
                .fetch_one(dht_store.db().pool())
                .await
                .unwrap();
        assert!(
            withheld_count > 0,
            "expected at least one ChainOpPublish row with withhold_publish=1 \
             after countersigning flush, got 0"
        );
    }

    /// `SourceChain::new` reads the chain head from the DHT store. After genesis
    /// plus one flush, a second `SourceChain::new` for the same author observes
    /// the flushed head as its persisted head.
    #[tokio::test(flavor = "multi_thread")]
    async fn chain_head_read_from_store() -> SourceChainResult<()> {
        let TestCase {
            chain,
            agent_key: alice,
            dht_store,
            keystore,
            ..
        } = TestCase::new().await;

        // Author one action and flush it so the DHT store has a head beyond genesis.
        let storage_arcs = vec![DhtArc::Empty];
        chain
            .put_weightless(
                builder::CloseChain { new_target: None },
                None,
                ChainTopOrdering::Strict,
            )
            .await?;
        let (flushed_actions, _) = chain.flush(storage_arcs).await?;
        let expected_head = flushed_actions
            .last()
            .expect("flush must return at least one action")
            .as_hash()
            .clone();

        let chain2 = SourceChain::new(dht_store.clone(), keystore, alice).await?;

        assert_eq!(
            chain2.persisted_head_info().map(|h| h.action),
            Some(expected_head),
        );

        Ok(())
    }

    /// The flush as-at check reads the store head. Two source chains for the
    /// same author share the store; once one flushes, the other's stale
    /// `persisted_head` must be detected as `HeadMoved`, and a normal flush's
    /// action must be visible via the store.
    #[tokio::test(flavor = "multi_thread")]
    async fn flush_as_at_detects_head_moved_against_store() -> SourceChainResult<()> {
        let TestCase {
            chain: chain_1,
            agent_key: alice,
            dht_store,
            keystore,
            ..
        } = TestCase::new().await;

        // A second chain reading the same store head; it goes stale once
        // chain_1 flushes.
        let chain_2 = SourceChain::new(dht_store.clone(), keystore.clone(), alice.clone()).await?;

        let action_builder = builder::CloseChain { new_target: None };
        chain_1
            .put(action_builder.clone(), None, ChainTopOrdering::Strict)
            .await?;
        chain_2
            .put(action_builder, None, ChainTopOrdering::Strict)
            .await?;

        // chain_1 flushes: its action becomes the store head and is visible via
        // the store.
        let (flushed, _) = chain_1.flush(vec![DhtArc::Empty]).await?;
        let flushed_head = flushed
            .last()
            .expect("flush returns at least one action")
            .as_hash()
            .clone();
        let store_head = dht_store
            .as_read()
            .chain_head_for_author(&alice)
            .await?
            .expect("store head present after flush");
        assert_eq!(store_head.action, flushed_head);

        // chain_2's persisted_head is stale; a strict flush must detect the
        // moved store head and fail with HeadMoved.
        assert_matches!(
            chain_2.flush(vec![DhtArc::Empty]).await,
            Err(SourceChainError::HeadMoved(_, _, _, _))
        );

        Ok(())
    }

    /// Concurrent strict flushes from the same chain head must not fork the
    /// chain. Two `SourceChain` handles for the same `(DNA, author)` observe the
    /// same store head, each stage one strict action, then flush in true
    /// parallel. The per-`(DNA, author)` chain write permit acquired in `flush`,
    /// combined with the as-at check against the store head, serializes the two
    /// flushes: exactly one commits and the other gets `HeadMoved`. The store
    /// head must advance by exactly one — never forking into two actions at the
    /// same sequence. Both flushing both successfully would be the fork bug the
    /// permit prevents, in which case this test fails.
    #[tokio::test(flavor = "multi_thread")]
    async fn concurrent_strict_flushes_do_not_fork_chain() -> SourceChainResult<()> {
        let TestCase {
            chain: _chain,
            agent_key: alice,
            dht_store,
            keystore,
            ..
        } = TestCase::new().await;

        // The head sequence both chains start contending from (the genesis head).
        let pre_seq = dht_store
            .as_read()
            .chain_head_for_author(&alice)
            .await?
            .expect("genesis chain head present")
            .seq;

        // Two fresh chains for the same author, both observing the same head.
        let chain_a = SourceChain::new(dht_store.clone(), keystore.clone(), alice.clone()).await?;
        let chain_b = SourceChain::new(dht_store.clone(), keystore.clone(), alice.clone()).await?;

        // Each stages one strict action from the same head; the flush loser gets
        // a clean `HeadMoved` (strict ordering means no relaxed rebase/retry).
        let action_builder = builder::CloseChain { new_target: None };
        chain_a
            .put(action_builder.clone(), None, ChainTopOrdering::Strict)
            .await?;
        chain_b
            .put(action_builder, None, ChainTopOrdering::Strict)
            .await?;

        // Flush both in true parallel on the multi-threaded runtime via spawned
        // tasks. `SourceChain` is `Clone` + `Send` + `'static`, so each owned
        // handle moves into its own task.
        let arcs_a = vec![DhtArc::Empty];
        let arcs_b = arcs_a.clone();
        let task_a = tokio::spawn(async move { chain_a.flush(arcs_a).await });
        let task_b = tokio::spawn(async move { chain_b.flush(arcs_b).await });
        let (res_a, res_b) = tokio::join!(task_a, task_b);
        let res_a = res_a.expect("flush task a did not panic");
        let res_b = res_b.expect("flush task b did not panic");

        // Exactly one flush committed (`Ok`) and exactly one was rejected with
        // `HeadMoved`. Don't assume which won the race.
        let oks = [&res_a, &res_b].iter().filter(|r| r.is_ok()).count();
        assert_eq!(
            oks, 1,
            "exactly one flush must commit; a={res_a:?}, b={res_b:?}"
        );
        let loser = if res_a.is_err() { &res_a } else { &res_b };
        assert_matches!(loser, Err(SourceChainError::HeadMoved(_, _, _, _)));

        // No fork: the store head advanced by exactly one.
        let new_seq = dht_store
            .as_read()
            .chain_head_for_author(&alice)
            .await?
            .expect("chain head present after flush")
            .seq;
        assert_eq!(
            new_seq,
            pre_seq + 1,
            "store head must advance by exactly one, not fork"
        );

        Ok(())
    }

    struct TestCase {
        chain: SourceChain,
        agent_key: AgentPubKey,
        dht_store: DhtStore,
        keystore: MetaLairClient,
    }

    impl TestCase {
        async fn new() -> Self {
            let keystore = test_keystore();
            let dna_hash = fixt!(DnaHash);
            let dht_store = crate::test_utils::test_dht_store(dna_hash.clone()).await;
            let agent_key = keystore.new_sign_keypair_random().await.unwrap();
            genesis(
                dht_store.clone(),
                keystore.clone(),
                dna_hash,
                agent_key.clone(),
                None,
            )
            .await
            .unwrap();
            let chain = SourceChain::new(dht_store.clone(), keystore.clone(), agent_key.clone())
                .await
                .unwrap();
            Self {
                chain,
                agent_key,
                dht_store,
                keystore,
            }
        }
    }

    async fn create_signed_warrant(
        author: &AgentPubKey,
        warrantee: &AgentPubKey,
        keystore: &MetaLairClient,
    ) -> SignedWarrant {
        let warrant = Warrant::new(
            WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
                action_author: warrantee.clone(),
                action: (fixt!(ActionHash), fixt!(Signature)),
                chain_op_type: ChainOpType::RegisterAgentActivity,
                reason: "invalid chain op".into(),
            }),
            author.clone(),
            Timestamp::now(),
            warrantee.clone(),
        );
        SignedWarrant::new(
            warrant.clone(),
            author.sign(keystore, warrant).await.unwrap(),
        )
    }
}
