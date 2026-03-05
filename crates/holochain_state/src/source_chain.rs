use crate::chain_lock::{get_chain_lock, ChainLock};
use crate::integrate::authored_ops_to_dht_db;
use crate::integrate::authored_ops_to_dht_db_without_check;
use crate::prelude::*;
use crate::query::chain_head::AuthoredChainHeadQuery;
use crate::scratch::ScratchError;
use crate::scratch::SyncScratchError;
use crate::source_chain;
use async_recursion::async_recursion;
pub use error::*;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::DhtOpHash;
use holo_hash::DnaHash;
use holo_hash::EntryHash;
use holo_hash::HasHash;
use holochain_keystore::MetaLairClient;
use holochain_sqlite::rusqlite;
use holochain_sqlite::rusqlite::params;
use holochain_sqlite::rusqlite::Transaction;
use holochain_sqlite::sql::sql_cell::SELECT_VALID_AGENT_PUB_KEY;
use holochain_sqlite::sql::sql_conductor::SELECT_VALID_CAP_GRANT_FOR_CAP_SECRET;
use holochain_sqlite::sql::sql_conductor::SELECT_VALID_UNRESTRICTED_CAP_GRANT;
use holochain_state_types::SourceChainDumpRecord;
use holochain_types::sql::AsSql;
use kitsune2_api::DhtArc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;

mod error;

#[derive(Clone)]
pub struct SourceChain<AuthorDb = DbWrite<DbKindAuthored>, DhtDb = DbWrite<DbKindDht>> {
    scratch: SyncScratch,
    vault: AuthorDb,
    dht_db: DhtDb,
    keystore: MetaLairClient,
    author: Arc<AgentPubKey>,
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

/// A source chain with read only access to the underlying databases.
pub type SourceChainRead = SourceChain<DbRead<DbKindAuthored>, DbRead<DbKindDht>>;

// TODO: document that many functions here are only reading from the scratch,
//       not the entire source chain!
/// Writable functions for a source chain with write access.
impl SourceChain {
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    pub async fn unlock_chain(&self) -> SourceChainResult<()> {
        self.vault
            .write_async({
                let author = self.author.clone();

                move |txn| unlock_chain(txn, &author)
            })
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

        let countersigning_agent_state = self
            .vault
            .write_async(move |txn| {
                // Check for a chain lock.
                // Note that the lock may not be valid anymore, but we must respect it here anyway.
                let chain_lock = get_chain_lock(txn, author.as_ref())?;
                if chain_lock.is_some() {
                    return Err(SourceChainError::ChainLocked);
                }
                let HeadInfo {
                    action: persisted_head,
                    seq: persisted_seq,
                    ..
                } = chain_head_db_nonempty(txn)?;
                let countersigning_agent_state =
                    CounterSigningAgentState::new(agent_index, persisted_head, persisted_seq);
                lock_chain(
                    txn,
                    author.as_ref(),
                    &hashed_preflight_request,
                    preflight_request.session_times.end(),
                )?;
                SourceChainResult::Ok(countersigning_agent_state)
            })
            .await?;
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
        let action = SignedActionHashed::sign(&self.keystore, action).await?;
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
    /// 3. Inserts entries, actions, and ops into the authored database in a single
    ///    transaction. Ops belonging to a countersigning session are marked as withheld
    ///    from publishing.
    /// 4. Integrates authored ops into the DHT database according to `storage_arcs`.
    /// 5. Verifies warrant signatures and inserts valid warrants into the DHT database.
    ///
    /// The `storage_arcs` should reflect the current target arcs for authored ops
    /// integration. Returns an empty vec with zero warrants if the scratch is empty.
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

        let ops_to_integrate = ops
            .iter()
            .map(|op| (op.1.clone(), op.0.dht_basis()))
            .collect::<Vec<_>>();

        // Write the entries, actions and ops to the database in one transaction.
        let author = self.author.clone();
        let persisted_head = self.head_info.as_ref().map(|h| h.action.clone());

        let now = Timestamp::now();

        // Take out a write lock as late as possible, after doing everything we can in memory and
        // before starting any database read/write operations.
        let write_permit = self.vault.acquire_write_permit().await?;

        // If there are records to write, then we need to respect the chain lock
        if !records.is_empty() {
            self.vault
                .read_async({
                    let author = author.clone();
                    move |txn| {
                        let chain_lock = get_chain_lock(txn, author.as_ref())?;
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
                                    return Err(
                                        SourceChainError::CountersigningWriteWithoutSession,
                                    );
                                }
                            }
                        }

                        Ok(())
                    }
                })
                .await?;
        }

        let chain_flush_result = self
            .vault
            .write_async_with_permit(write_permit, move |txn| {
                for scheduled_fn in scheduled_fns {
                    schedule_fn(txn, author.as_ref(), scheduled_fn, None, now)?;
                }

                if actions.last().is_none() {
                    // Nothing to write
                    return Ok(Vec::new());
                }

                // As at check.
                let head_info = chain_head_db(txn)?;
                let latest_head = head_info.as_ref().map(|h| h.action.clone());

                if persisted_head != latest_head {
                    return Err(SourceChainError::HeadMoved(
                        Box::new(actions),
                        Box::new(entries),
                        persisted_head,
                        head_info,
                    ));
                }

                for entry in entries {
                    insert_entry(txn, entry.as_hash(), entry.as_content())?;
                }
                for shh in actions.iter() {
                    insert_action(txn, shh)?;
                }
                for (op, op_hash, op_order, timestamp, _dep) in &ops {
                    insert_op_lite_into_authored(txn, op, op_hash, op_order, timestamp)?;
                    // If this is a countersigning session we want to withhold
                    // publishing the ops until the session is successful.
                    if is_countersigning_session {
                        set_withhold_publish(txn, op_hash)?;
                    }
                }
                SourceChainResult::Ok(actions)
            })
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
                        self.vault.clone(),
                        self.dht_db.clone(),
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
            Ok((actions, permit)) => {
                drop(permit);

                authored_ops_to_dht_db(
                    storage_arcs,
                    ops_to_integrate,
                    self.vault.clone().into(),
                    self.dht_db.clone(),
                )
                .await?;

                // Insert warrants into DHT database.
                // Check signatures first
                let mut warrants_to_insert = Vec::new();
                for warrant in warrants {
                    match warrant
                        .author
                        .verify_signature(warrant.signature(), warrant.data())
                        .await
                    {
                        Ok(true) => warrants_to_insert.push(warrant),
                        Ok(false) => {
                            tracing::info!(
                        "Invalid signature of a warrant in the scratch space. Skipping warrant"
                    );
                            continue;
                        }
                        Err(err) => {
                            tracing::warn!(?err, "Could not verify warrant signature before inserting from scratch space into DHT database. Skipping warrant");
                            continue;
                        }
                    }
                }

                // Write warrants to DHT database
                let total_inserted_warrants = self
                    .dht_db
                    .write_async(|txn| -> DatabaseResult<u32> {
                        let mut inserted_warrants = 0;
                        for warrant in warrants_to_insert {
                            let warrant_op = DhtOpHashed::from_content_sync(DhtOp::from(
                                WarrantOp::from(warrant),
                            ));
                            let serialized_size =
                                encode(&warrant_op).unwrap_or_else(|_| vec![]).len() as u32;
                            match insert_op_dht(txn, &warrant_op, serialized_size, None) {
                                // TODO: This should only be increased if the op has actually been inserted.
                                //       It's not possible to determine that, because mutations don't return
                                //       the row count. Fix this once row count is returned.
                                Ok(_) => inserted_warrants += 1,
                                Err(err) => {
                                    tracing::warn!(
                                        ?err,
                                        "Could not insert warrant from scratch space into DHT database"
                                    );

                                }
                            }
                        }
                        Ok(inserted_warrants)
                    })
                    .await
                    .unwrap(); // unwrap is safe here because no errors are returned from the closure

                SourceChainResult::Ok((actions, total_inserted_warrants))
            }
            Err(e) => Err(e),
        }
    }

    /// Checks if the current [`AgentPubKey`] of the source chain is valid and returns its [`Create`] action.
    ///
    /// Valid means that there's no [`Update`] or [`Delete`] action for the key on the chain.
    /// Returns the create action if it is valid, and an [`SourceChainError::InvalidAgentKey`] otherwise.
    pub async fn valid_create_agent_key_action(&self) -> SourceChainResult<Action> {
        let agent_key_entry_hash: EntryHash = self.agent_pubkey().clone().into();
        self.author_db()
            .read_async({
                let agent_key = self.agent_pubkey().clone();
                let cell_id = self.cell_id().as_ref().clone();
                move |txn| {
                    txn.query_row(
                        SELECT_VALID_AGENT_PUB_KEY,
                        named_params! {
                            ":author": agent_key.clone(),
                            ":type": ActionType::Create.to_string(),
                            ":entry_type": EntryType::AgentPubKey.to_string(),
                            ":entry_hash": agent_key_entry_hash
                        },
                        |row| {
                            let create_agent_signed_action = from_blob::<SignedAction>(row.get(0)?)
                                .map_err(|_| rusqlite::Error::BlobSizeError)?;
                            let create_agent_action = create_agent_signed_action.action().clone();
                            Ok(create_agent_action)
                        },
                    )
                    .map_err(|err| match err {
                        rusqlite::Error::BlobSizeError | rusqlite::Error::QueryReturnedNoRows => {
                            SourceChainError::InvalidAgentKey(agent_key, cell_id)
                        }
                        _ => {
                            tracing::error!(?err, "Error looking up valid agent pub key");
                            SourceChainError::other(err)
                        }
                    })
                }
            })
            .await
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

impl<AuthorDb, DhtDb> SourceChain<AuthorDb, DhtDb>
where
    AuthorDb: ReadAccess<DbKindAuthored>,
    DhtDb: ReadAccess<DbKindDht>,
{
    pub async fn new(
        vault: AuthorDb,
        dht_db: DhtDb,
        keystore: MetaLairClient,
        author: AgentPubKey,
    ) -> SourceChainResult<Self> {
        let scratch = Scratch::new().into_sync();
        let author = Arc::new(author);
        let head_info = Some(vault.read_async(chain_head_db_nonempty).await?);
        Ok(Self {
            scratch,
            vault,
            dht_db,
            keystore,
            author,
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
        vault: AuthorDb,
        dht_db: DhtDb,
        keystore: MetaLairClient,
        author: AgentPubKey,
    ) -> SourceChainResult<Self> {
        let scratch = Scratch::new().into_sync();
        let author = Arc::new(author);
        let head_info = vault.read_async(chain_head_db).await?;
        Ok(Self {
            scratch,
            vault,
            dht_db,
            keystore,
            author,
            head_info,
            public_only: false,
            zomes_initialized: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn public_only(&mut self) {
        self.public_only = true;
    }

    pub fn keystore(&self) -> &MetaLairClient {
        &self.keystore
    }

    pub fn author_db(&self) -> &AuthorDb {
        &self.vault
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
        self.vault.kind().0.clone()
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

        // remote caller
        let maybe_cap_grant = self
            .vault
            .read_async({
                let author = self.agent_pubkey().clone();
                move |txn| -> Result<_, DatabaseError> {
                    // closure to process resulting rows from query
                    let query_row_fn = |row: &Row| {
                        from_blob::<Entry>(row.get("blob")?)
                            .and_then(|entry| {
                                entry.as_cap_grant().ok_or_else(|| {
                                    crate::query::StateQueryError::SerializedBytesError(
                                        SerializedBytesError::Deserialize(
                                            "could not deserialize cap grant from entry"
                                                .to_string(),
                                        ),
                                    )
                                })
                            })
                            .map_err(|err| {
                                holochain_sqlite::rusqlite::Error::InvalidColumnType(
                                    0,
                                    err.to_string(),
                                    holochain_sqlite::rusqlite::types::Type::Blob,
                                )
                            })
                    };

                    // query cap grants depending on whether cap secret provided or not
                    let cap_grants = if let Some(cap_secret) = &check_secret {
                        let cap_secret_blob = to_blob(cap_secret).map_err(|err| {
                            DatabaseError::SerializedBytes(SerializedBytesError::Serialize(
                                err.to_string(),
                            ))
                        })?;

                        // cap grant for cap secret must exist
                        // that has not been updated or deleted
                        let mut stmt = txn.prepare(SELECT_VALID_CAP_GRANT_FOR_CAP_SECRET)?;
                        let rows = stmt.query(params![cap_secret_blob, author])?;
                        let cap_grant: Vec<CapGrant> = rows.map(query_row_fn).collect()?;
                        cap_grant
                    } else {
                        // unrestricted cap grant must exist
                        // that has not been updated or deleted
                        let mut stmt = txn.prepare(SELECT_VALID_UNRESTRICTED_CAP_GRANT)?;
                        let rows = stmt.query(params![CapAccess::Unrestricted.as_sql(), author])?;
                        let cap_grants: Vec<CapGrant> = rows.map(query_row_fn).collect()?;
                        cap_grants
                    };
                    // loop over all found cap grants and check if one of them
                    // is valid for assignee and function
                    for cap_grant in cap_grants {
                        if cap_grant.is_valid(&check_function, &check_agent, check_secret.as_ref())
                        {
                            return Ok(Some(cap_grant));
                        }
                    }
                    Ok(None)
                }
            })
            .await?;
        Ok(maybe_cap_grant)
    }

    /// Query Actions in the source chain.
    ///
    /// This returns a Vec rather than an iterator because it is intended to be
    /// used by the `query` host function, which crosses the wasm boundary.
    pub async fn query(&self, query: QueryFilter) -> SourceChainResult<Vec<Record>> {
        let public_only = self.public_only;

        let entry_type_filters_count = query.entry_type.as_ref().map_or(0, |t| t.len());
        let action_type_filters_count = query.action_type.as_ref().map_or(0, |t| t.len());

        let (scratch_seq_start, scratch_seq_end) =
            self.scratch.apply(|scratch| match &query.sequence_range {
                ChainQueryFilterRange::ActionHashRange(start, end) => {
                    let start_seq = scratch.actions().find_map(|a| {
                        if a.as_hash() == start {
                            Some(a.seq())
                        } else {
                            None
                        }
                    });
                    let end_seq = scratch.actions().find_map(|a| {
                        if a.as_hash() == end {
                            Some(a.seq())
                        } else {
                            None
                        }
                    });

                    (start_seq, end_seq)
                }
                ChainQueryFilterRange::ActionHashTerminated(end, _) => {
                    let end_seq = scratch.actions().find_map(|a| {
                        if a.as_hash() == end {
                            Some(a.seq())
                        } else {
                            None
                        }
                    });
                    (None, end_seq)
                }
                _ => (None, None),
            })?;

        let mut records = self
            .vault
            .read_async({
                let query = query.clone();
                move |txn| {
                    // This type is similar to what `named_params!` from rusqlite creates, except for the use of
                    // boxing to allow references to be passed to the query. The reserved capacity here should
                    // account for the number of parameters inserted below, including the variable inputs like
                    // entry_types and actions_types.
                    let mut args: Vec<(String, Box<dyn rusqlite::ToSql>)> = Vec::with_capacity(
                        6 + entry_type_filters_count + action_type_filters_count,
                    );

                    // Build the SELECT part of the query
                    let mut sql =
                        "SELECT DISTINCT Action.hash AS action_hash, Action.blob AS action_blob"
                            .to_string();
                    if query.include_entries {
                        sql.push_str(", Entry.blob AS entry_blob");
                    }

                    // Build the FROM and JOIN parts of the query
                    sql.push_str("\nFROM Action");
                    if query.include_entries {
                        sql.push_str("\nLEFT JOIN Entry On Action.entry_hash = Entry.hash");
                    }

                    match &query.sequence_range {
                        ChainQueryFilterRange::Unbounded => {
                            sql.push_str("\nWHERE 1=1");
                        }
                        ChainQueryFilterRange::ActionSeqRange(start, end) => {
                            args.push((":range_start".to_string(), Box::new(start)));
                            args.push((":range_end".to_string(), Box::new(end)));

                            sql.push_str("\nWHERE Action.seq BETWEEN :range_start AND :range_end");
                        }
                        ChainQueryFilterRange::ActionHashRange(
                            start_action_hash,
                            end_action_hash,
                        ) => {
                            let start_seq = match scratch_seq_start {
                                Some(scratch_start) => scratch_start,
                                None => txn.query_row(
                                    "SELECT seq from Action WHERE hash = :range_start_hash",
                                    named_params! {":range_start_hash": start_action_hash.clone()},
                                    |row| row.get::<_, u32>(0),
                                )?,
                            };
                            let end_seq = match scratch_seq_end {
                                Some(scratch_end) => scratch_end,
                                None => txn.query_row(
                                    "SELECT seq from Action WHERE hash = :range_end_hash",
                                    named_params! {":range_end_hash": end_action_hash.clone()},
                                    |row| row.get::<_, u32>(0),
                                )?,
                            };

                            sql.push_str(&format!(
                                "\nWHERE Action.seq BETWEEN {start_seq} AND {end_seq}"
                            ));
                        }
                        ChainQueryFilterRange::ActionHashTerminated(
                            end_action_hash,
                            prior_count,
                        ) => {
                            let end_seq = match scratch_seq_end {
                                Some(scratch_end) => scratch_end,
                                None => txn.query_row(
                                    "SELECT seq from Action WHERE hash = :range_end_hash",
                                    named_params! {":range_end_hash": end_action_hash.clone()},
                                    |row| row.get::<_, u32>(0),
                                )?,
                            };

                            let start_seq = end_seq.saturating_sub(*prior_count);

                            sql.push_str(&format!(
                                "\nWHERE Action.seq BETWEEN {start_seq} AND {end_seq}"
                            ));
                        }
                    }

                    match query.sequence_range {
                        ChainQueryFilterRange::Unbounded
                        | ChainQueryFilterRange::ActionSeqRange(_, _) => {
                            if let Some(action_types) = &query.action_type {
                                if !action_types.is_empty() {
                                    for (i, _) in action_types.iter().enumerate() {
                                        args.push((
                                            format!(":action_type_{i}"),
                                            Box::new(action_types[i].as_sql()),
                                        ));
                                    }

                                    sql.push_str(
                                        format!(
                                            "\nAND Action.type IN ({})",
                                            named_param_seq("action_type", action_types.len())
                                        )
                                        .as_str(),
                                    );
                                }
                            }

                            if let Some(entry_types) = &query.entry_type {
                                if !entry_types.is_empty() {
                                    for (i, _) in entry_types.iter().enumerate() {
                                        args.push((
                                            format!(":entry_type_{i}"),
                                            Box::new(entry_types[i].as_sql()),
                                        ));
                                    }

                                    sql.push_str(
                                        format!(
                                            "\nAND Action.entry_type IN ({})",
                                            named_param_seq("entry_type", entry_types.len())
                                        )
                                        .as_str(),
                                    );
                                }
                            }
                        }
                        _ => {}
                    }

                    sql.push_str("\nORDER BY Action.seq");
                    sql.push_str(if query.order_descending {
                        " DESC"
                    } else {
                        " ASC"
                    });
                    let mut stmt = txn.prepare(&sql)?;

                    let records = stmt
                        .query_and_then(
                            args.iter()
                                .map(|a| (a.0.as_str(), a.1.as_ref()))
                                .collect::<Vec<(&str, &dyn rusqlite::ToSql)>>()
                                .as_slice(),
                            |row| {
                                let action = from_blob::<SignedAction>(row.get("action_blob")?)?;
                                let (action, signature) = action.into();
                                let private_entry = action
                                    .entry_type()
                                    .is_some_and(|e| *e.visibility() == EntryVisibility::Private);
                                let hash: ActionHash = row.get("action_hash")?;
                                let action = ActionHashed::with_pre_hashed(action, hash);
                                let sah = SignedActionHashed::with_presigned(action, signature);
                                let entry =
                                    if query.include_entries && (!private_entry || !public_only) {
                                        let entry: Option<Vec<u8>> = row.get("entry_blob")?;
                                        match entry {
                                            Some(entry) => Some(from_blob::<Entry>(entry)?),
                                            None => None,
                                        }
                                    } else {
                                        None
                                    };
                                StateQueryResult::Ok(Record::new(sah, entry))
                            },
                        )?
                        .collect::<StateQueryResult<Vec<_>>>();
                    records
                }
            })
            .await?;

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
                    Some(Record::new(sah.clone(), entry))
                })
                .collect();
            scratch_records.sort_unstable_by_key(|e| e.action().action_seq());
            records.extend(scratch_records);
        })?;

        Ok(query.filter_records(records))
    }

    pub async fn get_chain_lock(&self) -> SourceChainResult<Option<ChainLock>> {
        let author = self.author.clone();
        Ok(self
            .vault
            .read_async(move |txn| get_chain_lock(txn, author.as_ref()))
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
        dump_state(self.author_db().clone().into(), (*self.author).clone()).await
    }
}

fn named_param_seq(base_name: &str, repeat: usize) -> String {
    if repeat == 0 {
        return String::new();
    }

    let mut seq = format!(":{base_name}");
    for i in 0..repeat {
        seq.push_str(format!(", :{base_name}_{i}").as_str());
    }

    seq
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

async fn rebase_actions_on(
    keystore: &MetaLairClient,
    mut actions: Vec<SignedActionHashed>,
    mut head: HeadInfo,
) -> Result<Vec<SignedActionHashed>, ScratchError> {
    actions.sort_by_key(|shh| shh.action().action_seq());
    for shh in actions.iter_mut() {
        let mut action = shh.action().clone();
        action.rebase_on(head.action.clone(), head.seq, head.timestamp)?;
        head.seq = action.action_seq();
        head.timestamp = action.timestamp();
        let hh = ActionHashed::from_content_sync(action);
        head.action = hh.as_hash().clone();
        let new_shh = SignedActionHashed::sign(keystore, hh).await?;
        *shh = new_shh;
    }
    Ok(actions)
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub async fn genesis(
    authored: DbWrite<DbKindAuthored>,
    dht_db: DbWrite<DbKindDht>,
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
    let dna_action = SignedActionHashed::sign(&keystore, dna_action).await?;
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
    let agent_validation_action =
        SignedActionHashed::sign(&keystore, agent_validation_action).await?;
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
    let agent_action = SignedActionHashed::sign(&keystore, agent_action).await?;
    let agent_record = Record::new(agent_action, Some(Entry::Agent(agent_pubkey.clone())));
    let agent_ops = produce_op_lites_from_records(vec![&agent_record])?;
    let (agent_action, agent_entry) = agent_record.clone().into_inner();
    let agent_entry = agent_entry.into_option();

    let mut ops_to_integrate = Vec::new();

    let ops_to_integrate = authored
        .write_async(move |txn| {
            ops_to_integrate.extend(source_chain::put_raw(txn, dna_action, dna_ops, None)?);
            ops_to_integrate.extend(source_chain::put_raw(
                txn,
                agent_validation_action,
                avh_ops,
                None,
            )?);
            ops_to_integrate.extend(source_chain::put_raw(
                txn,
                agent_action,
                agent_ops,
                agent_entry,
            )?);
            SourceChainResult::Ok(ops_to_integrate)
        })
        .await?;

    // We don't check for authorityship here because during genesis we have no opportunity
    // to discover that the network is sharded and that we should not be an authority for
    // these items, so we assume we are an authority.
    authored_ops_to_dht_db_without_check(ops_to_integrate, authored.clone().into(), dht_db).await?;
    Ok(())
}

/// Should only be used to put items into the Authored DB.
/// Hash transfer fields (source, transfer_method, transfer_time) are not set.
pub fn put_raw(
    txn: &mut Transaction,
    shh: SignedActionHashed,
    ops: Vec<ChainOpLite>,
    entry: Option<Entry>,
) -> StateMutationResult<Vec<DhtOpHash>> {
    let (action, signature) = shh.into_inner();
    let (action, hash) = action.into_inner();
    let mut action = Some(action);
    let mut hashes = Vec::with_capacity(ops.len());
    let mut ops_to_integrate = Vec::with_capacity(ops.len());
    for op in &ops {
        let op_type = op.get_type();
        let (h, op_hash) =
            ChainOpUniqueForm::op_hash(op_type, action.take().expect("This can't be empty"))?;
        let op_order = OpOrder::new(op_type, h.timestamp());
        let timestamp = h.timestamp();
        action = Some(h);
        hashes.push((op_hash.clone(), op_order, timestamp));
        ops_to_integrate.push(op_hash);
    }
    let shh = SignedActionHashed::with_presigned(
        ActionHashed::with_pre_hashed(action.expect("This can't be empty"), hash),
        signature,
    );
    if let Some(entry) = entry {
        insert_entry(txn, &EntryHash::with_data_sync(&entry), &entry)?;
    }
    insert_action(txn, &shh)?;
    for (op, (op_hash, op_order, timestamp)) in ops.into_iter().zip(hashes) {
        insert_op_lite(txn, &op.into(), &op_hash, &op_order, &timestamp, 0, None)?;
    }
    Ok(ops_to_integrate)
}

/// Get the current chain head of the database, if the chain is nonempty.
pub fn chain_head_db(txn: &Txn<DbKindAuthored>) -> SourceChainResult<Option<HeadInfo>> {
    let chain_head = AuthoredChainHeadQuery::new();
    Ok(chain_head.run(CascadeTxnWrapper::from(txn))?)
}

/// Get the current chain head of the database.
/// Error if the chain is empty.
pub fn chain_head_db_nonempty(txn: &Txn<DbKindAuthored>) -> SourceChainResult<HeadInfo> {
    chain_head_db(txn)?.ok_or(SourceChainError::ChainEmpty)
}

pub type CurrentCountersigningSessionOpt = Option<(Record, EntryHash, CounterSigningSessionData)>;

/// Check if there is a current countersigning session and if so, return the
/// session data and the entry hash.
pub fn current_countersigning_session(
    txn: &Txn<DbKindAuthored>,
) -> SourceChainResult<CurrentCountersigningSessionOpt> {
    match chain_head_db(txn) {
        // We haven't done genesis so no session can be active.
        Err(e) => Err(e),
        Ok(None) => Ok(None),
        Ok(Some(HeadInfo { action: hash, .. })) => {
            let txn: CascadeTxnWrapper = txn.into();
            // Get the session data from the database.
            let record = match txn.get_record(&hash.into())? {
                Some(record) => record,
                None => return Ok(None),
            };
            let (sah, ee) = record.clone().into_inner();
            Ok(match (sah.action().entry_hash(), ee.into_option()) {
                (Some(entry_hash), Some(Entry::CounterSign(cs, _))) => {
                    Some((record, entry_hash.clone(), *cs))
                }
                _ => None,
            })
        }
    }
}

/// dump the entire source chain as a pretty-printed json string
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub async fn dump_state(
    vault: DbRead<DbKindAuthored>,
    author: AgentPubKey,
) -> Result<SourceChainDump, SourceChainError> {
    Ok(vault
        .read_async(move |txn| {
            let records = txn
                .prepare(
                    "
                SELECT DISTINCT
                Action.blob AS action_blob, Entry.blob AS entry_blob,
                Action.hash AS action_hash
                FROM Action
                JOIN DhtOp ON DhtOp.action_hash = Action.hash
                LEFT JOIN Entry ON Action.entry_hash = Entry.hash
                WHERE
                Action.author = :author
                ORDER BY Action.seq ASC
                ",
                )?
                .query_and_then(
                    named_params! {
                        ":author": author,
                    },
                    |row| {
                        let action: SignedAction = from_blob(row.get("action_blob")?)?;
                        let (action, signature) = action.into();
                        let action_address = row.get("action_hash")?;
                        let entry: Option<Vec<u8>> = row.get("entry_blob")?;
                        let entry: Option<Entry> = match entry {
                            Some(entry) => Some(from_blob(entry)?),
                            None => None,
                        };
                        StateQueryResult::Ok(SourceChainDumpRecord {
                            signature,
                            action_address,
                            action,
                            entry,
                        })
                    },
                )?
                .collect::<StateQueryResult<Vec<_>>>()?;
            let published_ops_count = txn.query_row(
                "
                SELECT COUNT(DhtOp.hash) FROM DhtOp
                JOIN Action ON DhtOp.action_hash = Action.hash
                WHERE
                Action.author = :author
                AND
                last_publish_time IS NOT NULL
                ",
                named_params! {
                ":author": author,
                },
                |row| row.get(0),
            )?;
            StateQueryResult::Ok(SourceChainDump {
                records,
                published_ops_count,
            })
        })
        .await?)
}

impl From<SourceChain> for SourceChainRead {
    fn from(chain: SourceChain) -> Self {
        SourceChainRead {
            vault: chain.vault.into(),
            dht_db: chain.dht_db.into(),
            scratch: chain.scratch,
            keystore: chain.keystore,
            author: chain.author,
            head_info: chain.head_info,
            public_only: chain.public_only,
            zomes_initialized: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;
    use crate::source_chain::SourceChainResult;
    use ::fixt::fixt;
    use ::fixt::prelude::*;
    use holo_hash::fixt::DnaHashFixturator;
    use holo_hash::fixt::{ActionHashFixturator, AgentPubKeyFixturator};
    use holochain_keystore::test_keystore;
    use holochain_zome_types::Entry;
    use matches::assert_matches;
    use std::collections::{BTreeSet, HashSet};

    #[tokio::test(flavor = "multi_thread")]
    async fn test_relaxed_ordering() -> SourceChainResult<()> {
        let TestCase {
            chain: chain_1,
            agent_key: alice,
            authored: db,
            dht: dht_db,
            keystore,
        } = TestCase::new().await;

        let chain_2 =
            SourceChain::new(db.clone(), dht_db.to_db(), keystore.clone(), alice.clone()).await?;
        let chain_3 =
            SourceChain::new(db.clone(), dht_db.to_db(), keystore.clone(), alice.clone()).await?;

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
        let seq = db
            .write_async(move |txn| chain_head_db_nonempty(txn))
            .await?
            .seq;
        assert_eq!(seq, 3);

        assert!(matches!(
            chain_2.flush(storage_arcs.clone()).await,
            Err(SourceChainError::HeadMoved(_, _, _, _))
        ));
        let seq = db
            .write_async(move |txn| chain_head_db_nonempty(txn))
            .await?
            .seq;
        assert_eq!(seq, 3);

        chain_3.flush(storage_arcs).await?;
        let seq = db
            .write_async(move |txn| chain_head_db_nonempty(txn))
            .await?
            .seq;
        assert_eq!(seq, 4);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_relaxed_ordering_with_entry() -> SourceChainResult<()> {
        let TestCase {
            chain: chain_1,
            agent_key: alice,
            authored: db,
            dht: dht_db,
            keystore,
        } = TestCase::new().await;

        let chain_2 =
            SourceChain::new(db.clone(), dht_db.to_db(), keystore.clone(), alice.clone()).await?;
        let chain_3 =
            SourceChain::new(db.clone(), dht_db.to_db(), keystore.clone(), alice.clone()).await?;

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
        let seq = db
            .write_async(move |txn| chain_head_db_nonempty(txn))
            .await?
            .seq;
        assert_eq!(seq, 3);

        assert!(matches!(
            chain_2.flush(storage_arcs.clone()).await,
            Err(SourceChainError::HeadMoved(_, _, _, _))
        ));

        chain_3.flush(storage_arcs).await?;
        let head = db
            .write_async(move |txn| chain_head_db_nonempty(txn))
            .await?;

        // not equal since action hash change due to rebasing
        assert_ne!(head.action, old_h2);
        assert_eq!(head.seq, 4);

        db.read_async(move |txn| -> DatabaseResult<()> {
            // get the full record
            let store = CascadeTxnWrapper::from(txn);
            let h1_record_entry_fetched = store
                .get_record(&h1.clone().into())
                .expect("error retrieving")
                .expect("entry not found")
                .into_inner()
                .1;
            let h2_record_entry_fetched = store
                .get_record(&head.action.clone().into())
                .expect("error retrieving")
                .expect("entry not found")
                .into_inner()
                .1;
            assert_eq!(RecordEntry::Present(entry_1), h1_record_entry_fetched);
            assert_eq!(RecordEntry::Present(entry_2), h2_record_entry_fetched);

            Ok(())
        })
        .await?;

        Ok(())
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

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_cap_grant() -> SourceChainResult<()> {
        let TestCase {
            chain,
            agent_key: alice,
            authored: db,
            dht: dht_db,
            keystore,
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
                SourceChain::new(db.clone(), dht_db.to_db(), keystore.clone(), alice.clone())
                    .await?;
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
            source_chain::genesis(
                db.clone(),
                dht_db.to_db(),
                keystore.clone(),
                fake_dna_hash(1),
                carol.clone(),
                None,
            )
            .await
            .unwrap();
            let carol_chain =
                SourceChain::new(db.clone(), dht_db.clone(), keystore.clone(), carol.clone())
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
                SourceChain::new(db.clone(), dht_db.to_db(), keystore.clone(), alice.clone())
                    .await?;
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
                SourceChain::new(db.clone(), dht_db.to_db(), keystore.clone(), alice.clone())
                    .await?;
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
                source_chain::genesis(
                    db.clone(),
                    dht_db.to_db(),
                    keystore.clone(),
                    fake_dna_hash(1),
                    bob.clone(),
                    None,
                )
                .await
                .unwrap();
                let bob_chain =
                    SourceChain::new(db.clone(), dht_db.clone(), keystore.clone(), bob.clone())
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
                SourceChain::new(db.clone(), dht_db.to_db(), keystore.clone(), alice.clone())
                    .await?;
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
                SourceChain::new(db.clone(), dht_db.to_db(), keystore.clone(), alice.clone())
                    .await?;

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
        let test_db = test_authored_db();
        let dht_db = test_dht_db();
        let keystore = test_keystore();
        let vault = test_db.to_db();

        let author = Arc::new(keystore.new_sign_keypair_random().await.unwrap());

        vault
            .read_async({
                move |txn| -> DatabaseResult<()> {
                    assert_matches!(chain_head_db(txn), Ok(None));

                    Ok(())
                }
            })
            .await
            .unwrap();
        genesis(
            vault.clone(),
            dht_db.to_db(),
            keystore.clone(),
            fixt!(DnaHash),
            (*author).clone(),
            None,
        )
        .await
        .unwrap();

        let source_chain = SourceChain::new(
            vault.clone(),
            dht_db.to_db(),
            keystore.clone(),
            (*author).clone(),
        )
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

        vault
            .read_async({
                let check_h1 = h1.clone();
                let check_h2 = h2.clone();

                move |txn| -> DatabaseResult<()> {
                    assert_eq!(chain_head_db_nonempty(txn).unwrap().action, check_h2);
                    // get the full record
                    let store = CascadeTxnWrapper::from(txn);
                    let h1_record_fetched = store
                        .get_record(&check_h1.clone().into())
                        .expect("error retrieving")
                        .expect("entry not found");
                    let h2_record_fetched = store
                        .get_record(&check_h2.clone().into())
                        .expect("error retrieving")
                        .expect("entry not found");
                    assert_eq!(check_h1, *h1_record_fetched.action_address());
                    assert_eq!(check_h2, *h2_record_fetched.action_address());

                    Ok(())
                }
            })
            .await
            .unwrap();

        // check that you can iterate on the chain
        let source_chain = SourceChain::new(
            vault.clone(),
            dht_db.to_db(),
            keystore.clone(),
            (*author).clone(),
        )
        .await
        .unwrap();
        let res = source_chain.query(QueryFilter::new()).await.unwrap();
        assert_eq!(res.len(), 5);
        assert_eq!(*res[3].action_address(), h1);
        assert_eq!(*res[4].action_address(), h2);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn source_chain_buffer_dump_entries_json() -> SourceChainResult<()> {
        let TestCase {
            chain: _,
            agent_key,
            authored,
            ..
        } = TestCase::new().await;

        let json = dump_state(authored.clone().into(), agent_key.clone()).await?;
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
            let sig = alice.sign(&keystore, &action).await.unwrap();
            let signed_action = SignedActionHashed::from_content_sync((action.clone(), sig).into());

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
            let sig = alice.sign(&keystore, &action).await.unwrap();
            let signed_action = SignedActionHashed::from_content_sync((action, sig).into());

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
        desc_sorted.sort_by_key(|r| r.signed_action.action().action_seq());
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
    async fn flush_writes_warrants_to_dht_db() {
        let TestCase {
            chain,
            agent_key,
            dht,
            keystore,
            ..
        } = TestCase::new().await;
        let warrantee = fixt!(AgentPubKey);

        let warrantee_clone = warrantee.clone();
        let actual_warrants = dht.test_read(move |txn| {
            CascadeTxnWrapper::from(txn)
                .get_warrants_for_agent(&warrantee_clone, true)
                .unwrap()
        });
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

        // Flush should write warrants to DHT database
        let (actions, warrant_count) = chain.flush(vec![]).await.unwrap();
        assert!(actions.is_empty());
        assert_eq!(warrant_count, 1);

        // Check DHT database
        let actual_warrants = dht.test_read(move |txn| {
            CascadeTxnWrapper::from(txn)
                .get_warrants_for_agent(&warrantee, false)
                .unwrap()
        });
        assert_eq!(actual_warrants, vec![WarrantOp::from(signed_warrant)]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn duplicate_warrants_are_not_inserted_during_flush() {
        holochain_trace::test_run();
        let TestCase {
            chain,
            agent_key,
            dht,
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

        // Flush should write warrant to DHT database
        let (actions, warrant_count) = chain.flush(vec![]).await.unwrap();
        assert!(actions.is_empty());
        assert_eq!(warrant_count, 1);

        // Check DHT database
        let warrantee_clone = warrantee.clone();
        let actual_warrants = dht.test_read(move |txn| {
            CascadeTxnWrapper::from(txn)
                .get_warrants_for_agent(&warrantee_clone, false)
                .unwrap()
        });
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

        // Flush should not write duplicate warrant to DHT database
        let (actions, warrant_count) = chain.flush(vec![]).await.unwrap();
        assert!(actions.is_empty());
        assert_eq!(warrant_count, 1); // rejected inserts are not reported by the insertion method, so this will indicate 1

        // Check DHT database again
        let actual_warrants = dht.test_read(move |txn| {
            CascadeTxnWrapper::from(txn)
                .get_warrants_for_agent(&warrantee, false)
                .unwrap()
        });
        assert_eq!(actual_warrants, vec![WarrantOp::from(signed_warrant)]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn counterfeit_warrants_are_not_inserted_during_flush() {
        let TestCase {
            chain,
            agent_key,
            dht,
            ..
        } = TestCase::new().await;
        let warrantee = fixt!(AgentPubKey);

        // Create a counterfeit warrant
        let warrant = Warrant::new(
            WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
                action_author: warrantee.clone(),
                action: (fixt!(ActionHash), fixt!(Signature)),
                chain_op_type: ChainOpType::RegisterAgentActivity,
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

        // Check DHT database
        let warrantee_clone = warrantee.clone();
        let actual_warrants = dht.test_read(move |txn| {
            CascadeTxnWrapper::from(txn)
                .get_warrants_for_agent(&warrantee_clone, false)
                .unwrap()
        });
        assert!(actual_warrants.is_empty());
    }

    struct TestCase {
        chain: SourceChain,
        agent_key: AgentPubKey,
        authored: TestDb<DbKindAuthored>,
        dht: TestDb<DbKindDht>,
        keystore: MetaLairClient,
    }

    impl TestCase {
        async fn new() -> Self {
            let authored = test_authored_db();
            let dht = test_dht_db();
            let keystore = test_keystore();
            let dna_hash = fixt!(DnaHash);
            let agent_key = keystore.new_sign_keypair_random().await.unwrap();
            genesis(
                authored.to_db(),
                dht.to_db(),
                keystore.clone(),
                dna_hash,
                agent_key.clone(),
                None,
            )
            .await
            .unwrap();
            let chain = SourceChain::new(
                authored.to_db(),
                dht.to_db(),
                keystore.clone(),
                agent_key.clone(),
            )
            .await
            .unwrap();
            Self {
                chain,
                agent_key,
                authored,
                dht,
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
