use std::sync::Arc;

use crate::integrate::authored_ops_to_dht_db;
use crate::integrate::authored_ops_to_dht_db_without_check;
use crate::scratch::ScratchError;
use crate::scratch::SyncScratchError;
use async_recursion::async_recursion;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::DhtOpHash;
use holo_hash::DnaHash;
use holo_hash::HasHash;
use holochain_keystore::MetaLairClient;
use holochain_p2p::HolochainP2pDnaT;
use holochain_sqlite::rusqlite::Transaction;
use holochain_types::db::DbRead;
use holochain_types::db::DbWrite;
use holochain_types::db_cache::DhtDbQueryCache;
use holochain_types::dht_op::produce_op_lights_from_iter;
use holochain_types::dht_op::produce_op_lights_from_records;
use holochain_types::dht_op::DhtOp;
use holochain_types::dht_op::DhtOpLight;
use holochain_types::dht_op::OpOrder;
use holochain_types::dht_op::UniqueForm;
use holochain_types::record::SignedActionHashedExt;
use holochain_types::sql::AsSql;
use holochain_zome_types::action;
use holochain_zome_types::query::ChainQueryFilterRange;
use holochain_zome_types::Action;
use holochain_zome_types::ActionBuilder;
use holochain_zome_types::ActionBuilderCommon;
use holochain_zome_types::ActionExt;
use holochain_zome_types::ActionHashed;
use holochain_zome_types::ActionUnweighed;
use holochain_zome_types::CapAccess;
use holochain_zome_types::CapGrant;
use holochain_zome_types::CapSecret;
use holochain_zome_types::CellId;
use holochain_zome_types::ChainTopOrdering;
use holochain_zome_types::CounterSigningAgentState;
use holochain_zome_types::CounterSigningSessionData;
use holochain_zome_types::Entry;
use holochain_zome_types::EntryRateWeight;
use holochain_zome_types::EntryVisibility;
use holochain_zome_types::GrantedFunction;
use holochain_zome_types::MembraneProof;
use holochain_zome_types::PreflightRequest;
use holochain_zome_types::QueryFilter;
use holochain_zome_types::Record;
use holochain_zome_types::Signature;
use holochain_zome_types::SignedAction;
use holochain_zome_types::SignedActionHashed;
use holochain_zome_types::Timestamp;

use crate::chain_lock::is_chain_locked;
use crate::chain_lock::is_lock_expired;
use crate::prelude::*;
use crate::query::chain_head::ChainHeadQuery;
use crate::scratch::Scratch;
use crate::scratch::SyncScratch;
use holo_hash::EntryHash;
use holochain_serialized_bytes::prelude::*;

pub use error::*;

mod error;

#[derive(Clone)]
pub struct SourceChain<AuthorDb = DbWrite<DbKindAuthored>, DhtDb = DbWrite<DbKindDht>> {
    scratch: SyncScratch,
    vault: AuthorDb,
    dht_db: DhtDb,
    dht_db_cache: DhtDbQueryCache,
    keystore: MetaLairClient,
    author: Arc<AgentPubKey>,
    persisted_seq: u32,
    persisted_head: ActionHash,
    persisted_timestamp: Timestamp,
    public_only: bool,
}

/// A source chain with read only access to the underlying databases.
pub type SourceChainRead = SourceChain<DbRead<DbKindAuthored>, DbRead<DbKindDht>>;

// TODO fix this.  We shouldn't really have nil values but this would
// show if the database is corrupted and doesn't have a record
#[derive(Serialize, Debug, Clone, Deserialize)]
pub struct SourceChainJsonDump {
    pub records: Vec<SourceChainJsonRecord>,
    pub published_ops_count: usize,
}

#[derive(Serialize, Debug, Clone, Deserialize)]
pub struct SourceChainJsonRecord {
    pub signature: Signature,
    pub action_address: ActionHash,
    pub action: Action,
    pub entry: Option<Entry>,
}

// TODO: document that many functions here are only reading from the scratch,
//       not the entire source chain!
/// Writable functions for a source chain with write access.
impl SourceChain {
    pub async fn unlock_chain(&self) -> SourceChainResult<()> {
        let author = self.author.clone();
        self.vault
            .async_commit(move |txn| unlock_chain(txn, &author))
            .await?;
        Ok(())
    }

    pub async fn accept_countersigning_preflight_request(
        &self,
        preflight_request: PreflightRequest,
        agent_index: u8,
    ) -> SourceChainResult<CounterSigningAgentState> {
        let hashed_preflight_request = holo_hash::encode::blake2b_256(
            &holochain_serialized_bytes::encode(&preflight_request)?,
        );

        // This all needs to be ensured in a non-panicky way BEFORE calling into the source chain here.
        let author = self.author.clone();
        assert_eq!(
            *author,
            preflight_request.signing_agents[agent_index as usize].0
        );

        let countersigning_agent_state = self
            .vault
            .async_commit(move |txn| {
                if is_chain_locked(txn, &hashed_preflight_request, author.as_ref())? {
                    return Err(SourceChainError::ChainLocked);
                }
                let (persisted_head, persisted_seq, _) = chain_head_db(txn, author.clone())?;
                let countersigning_agent_state =
                    CounterSigningAgentState::new(agent_index, persisted_head, persisted_seq);
                lock_chain(
                    txn,
                    &hashed_preflight_request,
                    author.as_ref(),
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
        let (prev_action, chain_head_seq, chain_head_timestamp) = self.chain_head()?;
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

    #[cfg(feature = "test_utils")]
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

    #[async_recursion]
    pub async fn flush(
        &self,
        network: &(dyn HolochainP2pDnaT + Send + Sync),
    ) -> SourceChainResult<Vec<SignedActionHashed>> {
        // Nothing to write
        if self.scratch.apply(|s| s.is_empty())? {
            return Ok(Vec::new());
        }
        let (scheduled_fns, actions, ops, entries) = self.scratch.apply_and_then(|scratch| {
            let (actions, ops) =
                build_ops_from_actions(scratch.drain_actions().collect::<Vec<_>>())?;

            // Drain out any entries.
            let entries = scratch.drain_entries().collect::<Vec<_>>();
            let scheduled_fns = scratch.drain_scheduled_fns().collect::<Vec<_>>();
            SourceChainResult::Ok((scheduled_fns, actions, ops, entries))
        })?;

        let maybe_countersigned_entry = entries
            .iter()
            .map(|entry| entry.as_content())
            .find(|entry| matches!(entry, Entry::CounterSign(_, _)));

        if matches!(maybe_countersigned_entry, Some(Entry::CounterSign(_, _))) && actions.len() != 1
        {
            return Err(SourceChainError::DirtyCounterSigningWrite);
        }
        let lock = lock_for_entry(maybe_countersigned_entry)?;

        // If the lock isn't empty this is a countersigning session.
        let is_countersigning_session = !lock.is_empty();

        let ops_to_integrate = ops
            .iter()
            .map(|op| (op.1.clone(), op.0.dht_basis().clone()))
            .collect::<Vec<_>>();

        // Write the entries, actions and ops to the database in one transaction.
        let author = self.author.clone();
        let persisted_head = self.persisted_head.clone();
        match self
            .vault
            .async_commit(move |txn: &mut Transaction| {
                let now = Timestamp::now();
                for scheduled_fn in scheduled_fns {
                    schedule_fn(txn, author.as_ref(), scheduled_fn, None, now)?;
                }
                // As at check.
                let (new_persisted_head, new_head_seq, new_timestamp) =
                    chain_head_db(txn, author.clone())?;
                if actions.last().is_none() {
                    // Nothing to write
                    return Ok(Vec::new());
                }
                if persisted_head != new_persisted_head {
                    return Err(SourceChainError::HeadMoved(
                        actions,
                        entries,
                        Some(persisted_head),
                        Some((new_persisted_head, new_head_seq, new_timestamp)),
                    ));
                }

                if is_chain_locked(txn, &lock, author.as_ref())? {
                    return Err(SourceChainError::ChainLocked);
                }
                // If this is a countersigning session, and the chain is NOT
                // locked then either the session expired or the countersigning
                // entry being committed now is the correct one for the lock.
                else if is_countersigning_session {
                    // If the lock is expired then we can't write this countersigning session.
                    if is_lock_expired(txn, &lock, author.as_ref())? {
                        return Err(SourceChainError::LockExpired);
                    }
                }

                for entry in entries {
                    insert_entry(txn, entry.as_hash(), entry.as_content())?;
                }
                for shh in actions.iter() {
                    insert_action(txn, shh)?;
                }
                for (op, op_hash, op_order, timestamp, _) in &ops {
                    insert_op_lite_into_authored(txn, op, op_hash, op_order, timestamp)?;
                    // If this is a countersigning session we want to withhold
                    // publishing the ops until the session is successful.
                    if is_countersigning_session {
                        set_withhold_publish(txn, op_hash)?;
                    }
                }
                SourceChainResult::Ok(actions)
            })
            .await
        {
            Err(SourceChainError::HeadMoved(
                actions,
                entries,
                old_head,
                Some((new_persisted_head, new_head_seq, new_timestamp)),
            )) => {
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
                        self.dht_db_cache.clone(),
                        keystore.clone(),
                        (*self.author).clone(),
                    )
                    .await?;
                    let rebased_actions = rebase_actions_on(
                        &keystore,
                        actions,
                        new_persisted_head,
                        new_head_seq,
                        new_timestamp,
                    )
                    .await?;
                    child_chain.scratch.apply(move |scratch| {
                        for action in rebased_actions {
                            scratch.add_action(action, ChainTopOrdering::Relaxed);
                        }
                        for entry in entries {
                            scratch.add_entry(entry, ChainTopOrdering::Relaxed);
                        }
                    })?;
                    child_chain.flush(network).await
                } else {
                    Err(SourceChainError::HeadMoved(
                        actions,
                        entries,
                        old_head,
                        Some((new_persisted_head, new_head_seq, new_timestamp)),
                    ))
                }
            }
            Ok(actions) => {
                authored_ops_to_dht_db(
                    network,
                    ops_to_integrate,
                    &self.vault,
                    &self.dht_db,
                    &self.dht_db_cache,
                )
                .await?;
                SourceChainResult::Ok(actions)
            }
            result => result,
        }
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
        dht_db_cache: DhtDbQueryCache,
        keystore: MetaLairClient,
        author: AgentPubKey,
    ) -> SourceChainResult<Self> {
        let scratch = Scratch::new().into_sync();
        let author = Arc::new(author);
        let (persisted_head, persisted_seq, persisted_timestamp) = vault
            .async_reader({
                let author = author.clone();
                move |txn| chain_head_db(&txn, author)
            })
            .await?;
        Ok(Self {
            scratch,
            vault,
            dht_db,
            dht_db_cache,
            keystore,
            author,
            persisted_seq,
            persisted_head,
            persisted_timestamp,
            public_only: false,
        })
    }

    /// Create a source chain with a blank chain head.
    /// You probably don't want this.
    /// This type is only useful for when a source chain
    /// really needs to be constructed before genesis runs.
    pub async fn raw_empty(
        vault: AuthorDb,
        dht_db: DhtDb,
        dht_db_cache: DhtDbQueryCache,
        keystore: MetaLairClient,
        author: AgentPubKey,
    ) -> SourceChainResult<Self> {
        let scratch = Scratch::new().into_sync();
        let author = Arc::new(author);
        let (persisted_head, persisted_seq, persisted_timestamp) = vault
            .async_reader({
                let author = author.clone();
                move |txn| chain_head_db(&txn, author)
            })
            .await
            .unwrap_or_else(|_| {
                (
                    ActionHash::from_raw_32(vec![0u8; 32]),
                    0,
                    Timestamp::from_micros(0),
                )
            });
        Ok(Self {
            scratch,
            vault,
            dht_db,
            dht_db_cache,
            keystore,
            author,
            persisted_seq,
            persisted_head,
            persisted_timestamp,
            public_only: false,
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

    pub fn cell_id(&self) -> CellId {
        CellId::new(
            self.vault.kind().dna_hash().clone(),
            self.agent_pubkey().clone(),
        )
    }

    /// This has to clone all the data because we can't return
    /// references to constructed data.
    // TODO: Maybe we should store data as records in the scratch?
    // TODO: document that this is only the records in the SCRATCH, not the
    //       entire source chain!
    pub fn scratch_records(&self) -> SourceChainResult<Vec<Record>> {
        Ok(self.scratch.apply(|scratch| scratch.records().collect())?)
    }

    pub fn has_initialized(&self) -> SourceChainResult<bool> {
        Ok(self.len()? > 3)
    }

    pub fn is_empty(&self) -> SourceChainResult<bool> {
        Ok(self.len()? == 0)
    }

    /// Accessor for the chain head that will be used at flush time to check
    /// the "as at" for ordering integrity etc.
    pub fn persisted_chain_head(&self) -> (ActionHash, u32, Timestamp) {
        (
            self.persisted_head.clone(),
            self.persisted_seq,
            self.persisted_timestamp,
        )
    }

    pub fn chain_head(&self) -> SourceChainResult<(ActionHash, u32, Timestamp)> {
        // Check scratch for newer head.
        Ok(self.scratch.apply(|scratch| {
            scratch
                .chain_head()
                .unwrap_or_else(|| self.persisted_chain_head())
        })?)
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> SourceChainResult<u32> {
        Ok(self.scratch.apply(|scratch| {
            let scratch_max = scratch.chain_head().map(|(_, s, _)| s);
            scratch_max
                .map(|s| std::cmp::max(s, self.persisted_seq))
                .unwrap_or(self.persisted_seq)
                + 1
        })?)
    }
    pub async fn valid_cap_grant(
        &self,
        check_function: GrantedFunction,
        check_agent: AgentPubKey,
        check_secret: Option<CapSecret>,
    ) -> SourceChainResult<Option<CapGrant>> {
        let author_grant = CapGrant::from(self.agent_pubkey().clone());
        if author_grant.is_valid(&check_function, &check_agent, check_secret.as_ref()) {
            return Ok(Some(author_grant));
        }
        let author = self.author.clone();
        // TODO: SQL_PERF: This query could have a fast upper bound if we add indexes.
        let valid_cap_grant = self
            .vault
            .async_reader(move |txn| {
                let not_referenced_action = "
            SELECT COUNT(H_REF.hash)
            FROM Action AS H_REF
            JOIN DhtOp AS D_REF ON D_REF.action_hash = H_REF.hash
            WHERE
            H_REF.author = :author
            AND
            (H_REF.original_action_hash = Action.hash
            OR
            H_REF.deletes_action_hash = Action.hash)
            ";
                let sql = format!(
                    "
                SELECT DISTINCT Entry.blob
                FROM Entry
                JOIN Action ON Action.entry_hash = Entry.hash
                JOIN DhtOp ON Action.hash = DhtOp.action_hash
                WHERE
                Action.author = :author
                AND
                Entry.access_type IS NOT NULL
                AND
                ({}) = 0
                ",
                    not_referenced_action
                );
                txn.prepare(&sql)?
                    .query_and_then(
                        named_params! {
                            ":author": author,
                        },
                        |row| from_blob(row.get("blob")?),
                    )?
                    .filter_map(|result: StateQueryResult<Entry>| match result {
                        Ok(entry) => entry
                            .as_cap_grant()
                            .filter(|grant| !matches!(grant, CapGrant::ChainAuthor(_)))
                            .filter(|grant| {
                                grant.is_valid(&check_function, &check_agent, check_secret.as_ref())
                            })
                            .map(|cap| Some(Ok(cap)))
                            .unwrap_or(None),
                        Err(e) => Some(Err(e)),
                    })
                    // if there are still multiple grants, fold them down based on specificity
                    // authorship > assigned > transferable > unrestricted
                    .fold(
                        Ok(None),
                        |acc: StateQueryResult<Option<CapGrant>>, grant| {
                            let grant = grant?;
                            let acc = acc?;
                            let acc = match &grant {
                                CapGrant::RemoteAgent(zome_call_cap_grant) => {
                                    match &zome_call_cap_grant.access {
                                        CapAccess::Assigned { .. } => match &acc {
                                            Some(CapGrant::RemoteAgent(
                                                acc_zome_call_cap_grant,
                                            )) => {
                                                match acc_zome_call_cap_grant.access {
                                                    // an assigned acc takes precedence
                                                    CapAccess::Assigned { .. } => acc,
                                                    // current grant takes precedence over all other accs
                                                    _ => Some(grant),
                                                }
                                            }
                                            None => Some(grant),
                                            // authorship should be short circuit and filtered
                                            _ => unreachable!(),
                                        },
                                        CapAccess::Transferable { .. } => match &acc {
                                            Some(CapGrant::RemoteAgent(
                                                acc_zome_call_cap_grant,
                                            )) => {
                                                match acc_zome_call_cap_grant.access {
                                                    // an assigned acc takes precedence
                                                    CapAccess::Assigned { .. } => acc,
                                                    // transferable acc takes precedence
                                                    CapAccess::Transferable { .. } => acc,
                                                    // current grant takes preference over other accs
                                                    _ => Some(grant),
                                                }
                                            }
                                            None => Some(grant),
                                            // authorship should be short circuited and filtered by now
                                            _ => unreachable!(),
                                        },
                                        CapAccess::Unrestricted => match acc {
                                            Some(_) => acc,
                                            None => Some(grant),
                                        },
                                    }
                                }
                                // ChainAuthor should have short circuited and be filtered out already
                                _ => unreachable!(),
                            };
                            Ok(acc)
                        },
                    )
            })
            .await?;
        Ok(valid_cap_grant)
    }

    /// Query Actions in the source chain.
    /// This returns a Vec rather than an iterator because it is intended to be
    /// used by the `query` host function, which crosses the wasm boundary
    // FIXME: This query needs to be tested.
    pub async fn query(&self, query: QueryFilter) -> SourceChainResult<Vec<Record>> {
        if query.sequence_range != ChainQueryFilterRange::Unbounded
            && (query.action_type.is_some()
                || query.entry_type.is_some()
                || query.entry_hashes.is_some()
                || query.include_entries)
        {
            return Err(SourceChainError::UnsupportedQuery(query));
        }
        let author = self.author.clone();
        let public_only = self.public_only;
        let mut records = self
            .vault
            .async_reader({
                let query = query.clone();
                move |txn| {
                    let mut sql = "
                SELECT DISTINCT
                Action.hash AS action_hash, Action.blob AS action_blob
            "
                    .to_string();
                    if query.include_entries {
                        sql.push_str(
                            "
                    , Entry.blob AS entry_blob
                    ",
                        );
                    }
                    sql.push_str(
                        "
                FROM Action
                ",
                    );
                    if query.include_entries {
                        sql.push_str(
                            "
                    LEFT JOIN Entry On Action.entry_hash = Entry.hash
                    ",
                        );
                    }
                    sql.push_str(
                        "
                JOIN DhtOp On DhtOp.action_hash = Action.hash
                WHERE
                Action.author = :author
                AND
                (
                    (:range_start IS NULL AND :range_end IS NULL AND :range_start_hash IS NULL AND :range_end_hash IS NULL AND :range_prior_count IS NULL)
                ",
                    );
                    sql.push_str(match query.sequence_range {
                        ChainQueryFilterRange::Unbounded => "",
                        ChainQueryFilterRange::ActionSeqRange(_, _) => "
                        OR (Action.seq BETWEEN :range_start AND :range_end)",
                        ChainQueryFilterRange::ActionHashRange(_, _) => "
                        OR (
                            Action.seq BETWEEN
                            (SELECT Action.seq from Action WHERE Action.hash = :range_start_hash)
                            AND
                            (SELECT Action.seq from Action WHERE Action.hash = :range_end_hash)
                        )",
                        ChainQueryFilterRange::ActionHashTerminated(_, _) => "
                        OR (
                            Action.seq BETWEEN
                            (SELECT Action.seq from Action WHERE Action.hash = :range_end_hash) - :range_prior_count
                            AND
                            (SELECT Action.seq from Action WHERE Action.hash = :range_end_hash)
                        )",
                    });
                    sql.push_str(
                        "
                        )
                        AND
                        (:entry_type IS NULL OR Action.entry_type = :entry_type)
                        AND
                        (:action_type IS NULL OR Action.type = :action_type)
                        ORDER BY Action.seq 
                        ",
                    );
                    sql.push_str(if query.descending {" DESC"} else {" ASC"});
                    let mut stmt = txn.prepare(&sql)?;
                    let records = stmt
                        .query_and_then(
                        named_params! {
                                ":author": author.as_ref(),
                                ":entry_type": query.entry_type.as_sql(),
                                ":action_type": query.action_type.as_sql(),
                                ":range_start": match query.sequence_range {
                                    ChainQueryFilterRange::ActionSeqRange(start, _) => Some(start),
                                    _ => None,
                                },
                                ":range_end": match query.sequence_range {
                                    ChainQueryFilterRange::ActionSeqRange(_, end) => Some(end),
                                    _ => None,
                                },
                                ":range_start_hash": match &query.sequence_range {
                                    ChainQueryFilterRange::ActionHashRange(start_hash, _) => Some(start_hash.clone()),
                                    _ => None,
                                },
                                ":range_end_hash": match &query.sequence_range {
                                    ChainQueryFilterRange::ActionHashRange(_, end_hash)
                                    | ChainQueryFilterRange::ActionHashTerminated(end_hash, _) => Some(end_hash.clone()),
                                    _ => None,
                                },
                                ":range_prior_count": match query.sequence_range {
                                    ChainQueryFilterRange::ActionHashTerminated(_, prior_count) => Some(prior_count),
                                    _ => None,
                                },
                            },
                            |row| {
                                let action = from_blob::<SignedAction>(row.get("action_blob")?)?;
                                let SignedAction(action, signature) = action;
                                let private_entry = action
                                    .entry_type()
                                    .map_or(false, |e| *e.visibility() == EntryVisibility::Private);
                                let hash: ActionHash = row.get("action_hash")?;
                                let action = ActionHashed::with_pre_hashed(action, hash);
                                let shh = SignedActionHashed::with_presigned(action, signature);
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
                                StateQueryResult::Ok(Record::new(shh, entry))
                            },
                        )?
                        .collect::<StateQueryResult<Vec<_>>>();
                    records
                }
            })
            .await?;
        self.scratch.apply(|scratch| {
            let mut scratch_records: Vec<_> = scratch
                .actions()
                .filter_map(|shh| {
                    let entry = match shh.action().entry_hash() {
                        Some(eh) if query.include_entries => scratch.get_entry(eh).ok()?,
                        _ => None,
                    };
                    Some(Record::new(shh.clone(), entry))
                })
                .collect();
            scratch_records.sort_unstable_by_key(|e| e.action().action_seq());

            records.extend(scratch_records);
        })?;
        Ok(query.filter_records(records))
    }

    pub async fn is_chain_locked(&self, lock: Vec<u8>) -> SourceChainResult<bool> {
        let author = self.author.clone();
        Ok(self
            .vault
            .async_reader(move |txn| is_chain_locked(&txn, &lock, author.as_ref()))
            .await?)
    }

    /// If there is a countersigning session get the
    /// StoreEntry op to send to the entry authorities.
    pub fn countersigning_op(&self) -> SourceChainResult<Option<DhtOp>> {
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
                            Some(DhtOp::StoreEntry(
                                shh.signature().clone(),
                                shh.action().clone().try_into().ok()?,
                                Box::new((**entry).clone()),
                            ))
                        })
                })
        })?;
        Ok(r)
    }
}

pub fn lock_for_entry(entry: Option<&Entry>) -> SourceChainResult<Vec<u8>> {
    Ok(match entry {
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
    Vec<(DhtOpLight, DhtOpHash, OpOrder, Timestamp, Dependency)>,
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
        let ops_inner = produce_op_lights_from_iter(vec![item].into_iter())?;

        // Break apart the SignedActionHashed.
        let (action, sig) = shh.into_inner();
        let (action, hash) = action.into_inner();

        // We need to take the action by value and put it back each loop.
        let mut h = Some(action);
        for op in ops_inner {
            let op_type = op.get_type();
            // Action is required by value to produce the DhtOpHash.
            let (action, op_hash) = UniqueForm::op_hash(op_type, h.expect("This can't be empty"))?;
            let op_order = OpOrder::new(op_type, action.timestamp());
            let timestamp = action.timestamp();
            // Put the action back by value.
            let dependency = get_dependency(op_type, &action);
            h = Some(action);
            // Collect the DhtOpLight, DhtOpHash and OpOrder.
            ops.push((op, op_hash, op_order, timestamp, dependency));
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
    mut rebase_action: ActionHash,
    mut rebase_seq: u32,
    mut rebase_timestamp: Timestamp,
) -> Result<Vec<SignedActionHashed>, ScratchError> {
    actions.sort_by_key(|shh| shh.action().action_seq());
    for shh in actions.iter_mut() {
        let mut action = shh.action().clone();
        action.rebase_on(rebase_action.clone(), rebase_seq, rebase_timestamp)?;
        rebase_seq = action.action_seq();
        rebase_timestamp = action.timestamp();
        let hh = ActionHashed::from_content_sync(action);
        rebase_action = hh.as_hash().clone();
        let new_shh = SignedActionHashed::sign(keystore, hh).await?;
        *shh = new_shh;
    }
    Ok(actions)
}

pub async fn genesis(
    authored: DbWrite<DbKindAuthored>,
    dht_db: DbWrite<DbKindDht>,
    dht_db_cache: &DhtDbQueryCache,
    keystore: MetaLairClient,
    dna_hash: DnaHash,
    agent_pubkey: AgentPubKey,
    membrane_proof: Option<MembraneProof>,
) -> SourceChainResult<()> {
    let dna_action = Action::Dna(action::Dna {
        author: agent_pubkey.clone(),
        timestamp: Timestamp::now(),
        hash: dna_hash,
    });
    let dna_action = ActionHashed::from_content_sync(dna_action);
    let dna_action = SignedActionHashed::sign(&keystore, dna_action).await?;
    let dna_action_address = dna_action.as_hash().clone();
    let record = Record::new(dna_action, None);
    let dna_ops = produce_op_lights_from_records(vec![&record])?;
    let (dna_action, _) = record.into_inner();

    // create the agent validation entry and add it directly to the store
    let agent_validation_action = Action::AgentValidationPkg(action::AgentValidationPkg {
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
    let record = Record::new(agent_validation_action, None);
    let avh_ops = produce_op_lights_from_records(vec![&record])?;
    let (agent_validation_action, _) = record.into_inner();

    // create a agent chain record and add it directly to the store
    let agent_action = Action::Create(action::Create {
        author: agent_pubkey.clone(),
        timestamp: Timestamp::now(),
        action_seq: 2,
        prev_action: avh_addr,
        entry_type: action::EntryType::AgentPubKey,
        entry_hash: agent_pubkey.clone().into(),
        // AgentPubKey is weightless
        weight: Default::default(),
    });
    let agent_action = ActionHashed::from_content_sync(agent_action);
    let agent_action = SignedActionHashed::sign(&keystore, agent_action).await?;
    let record = Record::new(agent_action, Some(Entry::Agent(agent_pubkey)));
    let agent_ops = produce_op_lights_from_records(vec![&record])?;
    let (agent_action, agent_entry) = record.into_inner();
    let agent_entry = agent_entry.into_option();

    let mut ops_to_integrate = Vec::new();

    let ops_to_integrate = authored
        .async_commit(move |txn| {
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
    authored_ops_to_dht_db_without_check(ops_to_integrate, &authored, &dht_db, dht_db_cache)
        .await?;
    Ok(())
}

pub fn put_raw(
    txn: &mut Transaction,
    shh: SignedActionHashed,
    ops: Vec<DhtOpLight>,
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
            UniqueForm::op_hash(op_type, action.take().expect("This can't be empty"))?;
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
        insert_op_lite(txn, &op, &op_hash, &op_order, &timestamp)?;
    }
    Ok(ops_to_integrate)
}

/// Get the current chain head of the database.
pub fn chain_head_db(
    txn: &Transaction,
    author: Arc<AgentPubKey>,
) -> SourceChainResult<(ActionHash, u32, Timestamp)> {
    let chain_head = ChainHeadQuery::new(author);
    let (prev_action, last_action_seq, last_action_timestamp) = chain_head
        .run(Txn::from(txn))?
        .ok_or(SourceChainError::ChainEmpty)?;
    Ok((prev_action, last_action_seq, last_action_timestamp))
}

/// Check if there is a current countersigning session and if so, return the
/// session data and the entry hash.
pub fn current_countersigning_session(
    txn: &Transaction<'_>,
    author: Arc<AgentPubKey>,
) -> SourceChainResult<Option<(EntryHash, CounterSigningSessionData)>> {
    // The chain must be locked for a session to be active.
    if is_chain_locked(txn, &[], author.as_ref())? {
        match chain_head_db(txn, author) {
            // We haven't done genesis so no session can be active.
            Err(SourceChainError::ChainEmpty) => Ok(None),
            Err(e) => Err(e),
            Ok((hash, _, _)) => {
                let txn: Txn = txn.into();
                // Get the session data from the database.
                let record = match txn.get_record(&hash.into())? {
                    Some(record) => record,
                    None => return Ok(None),
                };
                let (shh, ee) = record.into_inner();
                Ok(match (shh.action().entry_hash(), ee.into_option()) {
                    (Some(entry_hash), Some(Entry::CounterSign(cs, _))) => {
                        Some((entry_hash.to_owned(), *cs))
                    }
                    _ => None,
                })
            }
        }
    } else {
        Ok(None)
    }
}

#[cfg(test)]
async fn _put_db<H: holochain_zome_types::ActionUnweighed, B: ActionBuilder<H>>(
    vault: holochain_types::db::DbWrite<DbKindAuthored>,
    keystore: &MetaLairClient,
    author: Arc<AgentPubKey>,
    action_builder: B,
    maybe_entry: Option<Entry>,
) -> SourceChainResult<ActionHash> {
    let (prev_action, last_action_seq, _) =
        fresh_reader_test!(vault, |txn| { chain_head_db(&txn, author.clone()) })?;
    let action_seq = last_action_seq + 1;

    let common = ActionBuilderCommon {
        author: (*author).clone(),
        timestamp: Timestamp::now(),
        action_seq,
        prev_action: prev_action.clone(),
    };
    let action = action_builder.build(common).weightless().into();
    let action = ActionHashed::from_content_sync(action);
    let action = SignedActionHashed::sign(keystore, action).await?;
    let record = Record::new(action, maybe_entry);
    let ops = produce_op_lights_from_records(vec![&record])?;
    let (action, entry) = record.into_inner();
    let entry = entry.into_option();
    let hash = action.as_hash().clone();
    vault.conn()?.with_commit_sync(|txn: &mut Transaction| {
        let (new_head, new_seq, new_timestamp) = chain_head_db(txn, author.clone())?;
        if new_head != prev_action {
            let entries = match (entry, action.action().entry_hash()) {
                (Some(e), Some(entry_hash)) => {
                    vec![holochain_types::EntryHashed::with_pre_hashed(
                        e,
                        entry_hash.clone(),
                    )]
                }
                _ => vec![],
            };
            return Err(SourceChainError::HeadMoved(
                vec![action],
                entries,
                Some(prev_action),
                Some((new_head, new_seq, new_timestamp)),
            ));
        }
        SourceChainResult::Ok(put_raw(txn, action, ops, entry)?)
    })?;
    Ok(hash)
}

/// dump the entire source chain as a pretty-printed json string
pub async fn dump_state(
    vault: DbRead<DbKindAuthored>,
    author: AgentPubKey,
) -> Result<SourceChainJsonDump, SourceChainError> {
    Ok(vault
        .async_reader(move |txn| {
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
                        let SignedAction(action, signature) = from_blob(row.get("action_blob")?)?;
                        let action_address = row.get("action_hash")?;
                        let entry: Option<Vec<u8>> = row.get("entry_blob")?;
                        let entry: Option<Entry> = match entry {
                            Some(entry) => Some(from_blob(entry)?),
                            None => None,
                        };
                        StateQueryResult::Ok(SourceChainJsonRecord {
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
            StateQueryResult::Ok(SourceChainJsonDump {
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
            dht_db_cache: chain.dht_db_cache,
            scratch: chain.scratch,
            keystore: chain.keystore,
            author: chain.author,
            persisted_seq: chain.persisted_seq,
            persisted_head: chain.persisted_head,
            persisted_timestamp: chain.persisted_timestamp,
            public_only: chain.public_only,
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::prelude::*;
    use ::fixt::prelude::*;
    use hdk::prelude::*;
    use holochain_p2p::MockHolochainP2pDnaT;
    use matches::assert_matches;

    use crate::source_chain::SourceChainResult;
    use holochain_zome_types::Entry;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_relaxed_ordering() -> SourceChainResult<()> {
        let test_db = test_authored_db();
        let dht_db = test_dht_db();
        let keystore = test_keystore();
        let db = test_db.to_db();
        let alice = fixt!(AgentPubKey, Predictable, 0);

        let mut mock = MockHolochainP2pDnaT::new();
        mock.expect_authority_for_hash().returning(|_| Ok(false));
        let dht_db_cache = DhtDbQueryCache::new(dht_db.to_db().into());

        source_chain::genesis(
            db.clone(),
            dht_db.to_db(),
            &dht_db_cache,
            keystore.clone(),
            fake_dna_hash(1),
            alice.clone(),
            None,
        )
        .await
        .unwrap();
        let chain_1 = SourceChain::new(
            db.clone().into(),
            dht_db.to_db(),
            dht_db_cache.clone(),
            keystore.clone(),
            alice.clone(),
        )
        .await?;
        let chain_2 = SourceChain::new(
            db.clone().into(),
            dht_db.to_db(),
            dht_db_cache.clone(),
            keystore.clone(),
            alice.clone(),
        )
        .await?;
        let chain_3 = SourceChain::new(
            db.clone().into(),
            dht_db.to_db(),
            dht_db_cache.clone(),
            keystore.clone(),
            alice.clone(),
        )
        .await?;

        let action_builder = builder::CloseChain {
            new_dna_hash: fixt!(DnaHash),
        };
        chain_1
            .put(action_builder.clone(), None, ChainTopOrdering::Strict)
            .await?;
        chain_2
            .put(action_builder.clone(), None, ChainTopOrdering::Strict)
            .await?;
        chain_3
            .put(action_builder, None, ChainTopOrdering::Relaxed)
            .await?;

        let author = Arc::new(alice);
        chain_1.flush(&mock).await?;
        let author_1 = Arc::clone(&author);
        let (_, seq, _) = db
            .async_commit(move |txn: &mut Transaction| chain_head_db(&txn, author_1))
            .await?;
        assert_eq!(seq, 3);

        assert!(matches!(
            chain_2.flush(&mock).await,
            Err(SourceChainError::HeadMoved(_, _, _, _))
        ));
        let author_2 = Arc::clone(&author);
        let (_, seq, _) = db
            .async_commit(move |txn: &mut Transaction| chain_head_db(&txn, author_2))
            .await?;
        assert_eq!(seq, 3);

        chain_3.flush(&mock).await?;
        let author_3 = Arc::clone(&author);
        let (_, seq, _) = db
            .async_commit(move |txn: &mut Transaction| chain_head_db(&txn, author_3))
            .await?;
        assert_eq!(seq, 4);

        Ok(())
    }
    #[tokio::test(flavor = "multi_thread")]
    async fn test_relaxed_ordering_with_entry() -> SourceChainResult<()> {
        let test_db = test_authored_db();
        let dht_db = test_dht_db();
        let keystore = test_keystore();
        let db = test_db.to_db();
        let alice = fixt!(AgentPubKey, Predictable, 0);

        let mut mock = MockHolochainP2pDnaT::new();
        mock.expect_authority_for_hash().returning(|_| Ok(false));
        let dht_db_cache = DhtDbQueryCache::new(dht_db.to_db().into());

        source_chain::genesis(
            db.clone(),
            dht_db.to_db(),
            &dht_db_cache,
            keystore.clone(),
            fake_dna_hash(1),
            alice.clone(),
            None,
        )
        .await
        .unwrap();

        let chain_1 = SourceChain::new(
            db.clone().into(),
            dht_db.to_db(),
            dht_db_cache.clone(),
            keystore.clone(),
            alice.clone(),
        )
        .await?;
        let chain_2 = SourceChain::new(
            db.clone().into(),
            dht_db.to_db(),
            dht_db_cache.clone(),
            keystore.clone(),
            alice.clone(),
        )
        .await?;
        let chain_3 = SourceChain::new(
            db.clone().into(),
            dht_db.to_db(),
            dht_db_cache.clone(),
            keystore.clone(),
            alice.clone(),
        )
        .await?;

        let entry_1 = Entry::App(fixt!(AppEntryBytes));
        let eh1 = EntryHash::with_data_sync(&entry_1);
        let create = builder::Create {
            entry_type: EntryType::App(fixt!(AppEntryType)),
            entry_hash: eh1.clone(),
        };
        let h1 = chain_1
            .put_weightless(create, Some(entry_1.clone()), ChainTopOrdering::Strict)
            .await
            .unwrap();

        let entry_err = Entry::App(fixt!(AppEntryBytes));
        let entry_hash_err = EntryHash::with_data_sync(&entry_err);
        let create = builder::Create {
            entry_type: EntryType::App(fixt!(AppEntryType)),
            entry_hash: entry_hash_err.clone(),
        };
        chain_2
            .put_weightless(create, Some(entry_err.clone()), ChainTopOrdering::Strict)
            .await
            .unwrap();

        let entry_2 = Entry::App(fixt!(AppEntryBytes));
        let eh2 = EntryHash::with_data_sync(&entry_2);
        let create = builder::Create {
            entry_type: EntryType::App(AppEntryType::new(
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

        let author = Arc::new(alice);
        chain_1.flush(&mock).await?;
        let author_1 = Arc::clone(&author);
        let (_, seq, _) = db
            .async_commit(move |txn: &mut Transaction| chain_head_db(&txn, author_1))
            .await?;
        assert_eq!(seq, 3);

        assert!(matches!(
            chain_2.flush(&mock).await,
            Err(SourceChainError::HeadMoved(_, _, _, _))
        ));

        chain_3.flush(&mock).await?;
        let author_2 = Arc::clone(&author);
        let (h2, seq, _) = db
            .async_commit(move |txn: &mut Transaction| chain_head_db(&txn, author_2.clone()))
            .await?;

        // not equal since action hash change due to rebasing
        assert_ne!(h2, old_h2);
        assert_eq!(seq, 4);

        fresh_reader_test!(db, |txn| {
            // get the full record
            let store = Txn::from(&txn);
            let h1_record_entry_fetched = store
                .get_record(&h1.clone().into())
                .expect("error retrieving")
                .expect("entry not found")
                .into_inner()
                .1;
            let h2_record_entry_fetched = store
                .get_record(&h2.clone().into())
                .expect("error retrieving")
                .expect("entry not found")
                .into_inner()
                .1;
            assert_eq!(RecordEntry::Present(entry_1), h1_record_entry_fetched);
            assert_eq!(RecordEntry::Present(entry_2), h2_record_entry_fetched);
        });

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_cap_grant() -> SourceChainResult<()> {
        let test_db = test_authored_db();
        let dht_db = test_dht_db();
        let dht_db_cache = DhtDbQueryCache::new(dht_db.to_db().into());
        let keystore = test_keystore();
        let db = test_db.to_db();
        let secret = Some(CapSecretFixturator::new(Unpredictable).next().unwrap());
        let access = CapAccess::from(secret.unwrap());
        let mut mock = MockHolochainP2pDnaT::new();
        mock.expect_authority_for_hash().returning(|_| Ok(false));

        // @todo curry
        let _curry = CurryPayloadsFixturator::new(Empty).next().unwrap();
        let function: GrantedFunction = ("foo".into(), "bar".into());
        let mut functions: GrantedFunctions = BTreeSet::new();
        functions.insert(function.clone());
        let grant = ZomeCallCapGrant::new("tag".into(), access.clone(), functions.clone());
        let mut agents = AgentPubKeyFixturator::new(Predictable);
        let alice = agents.next().unwrap();
        let bob = agents.next().unwrap();
        source_chain::genesis(
            db.clone(),
            dht_db.to_db(),
            &dht_db_cache,
            keystore.clone(),
            fake_dna_hash(1),
            alice.clone(),
            None,
        )
        .await
        .unwrap();

        {
            let chain = SourceChain::new(
                db.clone(),
                dht_db.to_db(),
                dht_db_cache.clone(),
                keystore.clone(),
                alice.clone(),
            )
            .await?;
            assert_eq!(
                chain
                    .valid_cap_grant(function.clone(), alice.clone(), secret.clone())
                    .await?,
                Some(CapGrant::ChainAuthor(alice.clone())),
            );

            // bob should not match anything as the secret hasn't been committed yet
            assert_eq!(
                chain
                    .valid_cap_grant(function.clone(), bob.clone(), secret.clone())
                    .await?,
                None
            );
        }

        let (original_action_address, original_entry_address) = {
            let chain = SourceChain::new(
                db.clone().into(),
                dht_db.to_db(),
                dht_db_cache.clone(),
                keystore.clone(),
                alice.clone(),
            )
            .await?;
            let (entry, entry_hash) =
                EntryHashed::from_content_sync(Entry::CapGrant(grant.clone())).into_inner();
            let action_builder = builder::Create {
                entry_type: EntryType::CapGrant,
                entry_hash: entry_hash.clone(),
            };
            let action = chain
                .put_weightless(action_builder, Some(entry), ChainTopOrdering::default())
                .await?;

            chain.flush(&mock).await.unwrap();

            (action, entry_hash)
        };

        {
            let chain = SourceChain::new(
                db.clone(),
                dht_db.to_db(),
                dht_db_cache.clone(),
                keystore.clone(),
                alice.clone(),
            )
            .await?;
            // alice should find her own authorship with higher priority than the committed grant
            // even if she passes in the secret
            assert_eq!(
                chain
                    .valid_cap_grant(function.clone(), alice.clone(), secret.clone())
                    .await?,
                Some(CapGrant::ChainAuthor(alice.clone())),
            );

            // bob should be granted with the committed grant as it matches the secret he passes to
            // alice at runtime
            assert_eq!(
                chain
                    .valid_cap_grant(function.clone(), bob.clone(), secret.clone())
                    .await?,
                Some(grant.clone().into())
            );
        }

        // let's roll the secret and assign the grant to bob specifically
        let mut assignees = BTreeSet::new();
        assignees.insert(bob.clone());
        let updated_secret = Some(CapSecretFixturator::new(Unpredictable).next().unwrap());
        let updated_access = CapAccess::from((updated_secret.clone().unwrap(), assignees));
        let updated_grant = ZomeCallCapGrant::new("tag".into(), updated_access.clone(), functions);

        let (updated_action_hash, updated_entry_hash) = {
            let chain = SourceChain::new(
                db.clone().into(),
                dht_db.to_db(),
                dht_db_cache.clone(),
                keystore.clone(),
                alice.clone(),
            )
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

            chain.flush(&mock).await.unwrap();

            (action, entry_hash)
        };

        {
            let chain = SourceChain::new(
                db.clone(),
                dht_db.to_db(),
                dht_db_cache.clone(),
                keystore.clone(),
                alice.clone(),
            )
            .await?;
            // alice should find her own authorship with higher priority than the committed grant
            // even if she passes in the secret
            assert_eq!(
                chain
                    .valid_cap_grant(function.clone(), alice.clone(), secret.clone())
                    .await?,
                Some(CapGrant::ChainAuthor(alice.clone())),
            );
            assert_eq!(
                chain
                    .valid_cap_grant(function.clone(), alice.clone(), updated_secret.clone())
                    .await?,
                Some(CapGrant::ChainAuthor(alice.clone())),
            );

            // bob MUST provide the updated secret as the old one is invalidated by the new one
            assert_eq!(
                chain
                    .valid_cap_grant(function.clone(), bob.clone(), secret.clone())
                    .await?,
                None
            );
            assert_eq!(
                chain
                    .valid_cap_grant(function.clone(), bob.clone(), updated_secret.clone())
                    .await?,
                Some(updated_grant.into())
            );
        }

        {
            let chain = SourceChain::new(
                db.clone().into(),
                dht_db.to_db(),
                dht_db_cache.clone(),
                keystore.clone(),
                alice.clone(),
            )
            .await?;
            let action_builder = builder::Delete {
                deletes_address: updated_action_hash,
                deletes_entry_address: updated_entry_hash,
            };
            chain
                .put_weightless(action_builder, None, ChainTopOrdering::default())
                .await?;

            chain.flush(&mock).await.unwrap();
        }

        {
            let chain = SourceChain::new(
                db.clone(),
                dht_db.to_db(),
                dht_db_cache.clone(),
                keystore.clone(),
                alice.clone(),
            )
            .await?;
            // alice should find her own authorship
            assert_eq!(
                chain
                    .valid_cap_grant(function.clone(), alice.clone(), secret.clone())
                    .await?,
                Some(CapGrant::ChainAuthor(alice.clone())),
            );
            assert_eq!(
                chain
                    .valid_cap_grant(function.clone(), alice.clone(), updated_secret.clone())
                    .await?,
                Some(CapGrant::ChainAuthor(alice)),
            );

            // bob has no access
            assert_eq!(
                chain
                    .valid_cap_grant(function.clone(), bob.clone(), secret.clone())
                    .await?,
                None
            );
            assert_eq!(
                chain
                    .valid_cap_grant(function.clone(), bob.clone(), updated_secret.clone())
                    .await?,
                None
            );
        }

        Ok(())
    }

    // @todo bring all this back when we want to administer cap claims better
    // #[tokio::test(flavor = "multi_thread")]
    // async fn test_get_cap_claim() -> SourceChainResult<()> {
    //     let test_db = test_cell_db();
    //     let db = test_db.db();
    //     let db = db.conn().unwrap().await;
    //     let secret = CapSecretFixturator::new(Unpredictable).next().unwrap();
    //     let agent_pubkey = fake_agent_pubkey_1().into();
    //     let claim = CapClaim::new("tag".into(), agent_pubkey, secret.clone());
    //     {
    //         let mut store = SourceChainBuf::new(db.clone().into(), &db).await?;
    //         store
    //             .genesis(fake_dna_hash(1), fake_agent_pubkey_1(), None)
    //             .await?;
    //         arc.conn().unwrap().with_commit(|writer| store.flush_to_txn(writer))?;
    //     }
    //
    //     {
    //         let mut chain = SourceChain::new(db.clone().into(), &db).await?;
    //         chain.put_cap_claim(claim.clone()).await?;
    //
    // // ideally the following would work, but it won't because currently
    // // we can't get claims from the scratch space
    // // this will be fixed once we add the capability index
    //
    // // assert_eq!(
    // //     chain.get_persisted_cap_claim_by_secret(&secret)?,
    // //     Some(claim.clone())
    // // );
    //
    //         arc.conn().unwrap().with_commit(|writer| chain.flush_to_txn(writer))?;
    //     }
    //
    //     {
    //         let chain = SourceChain::new(db.clone().into(), &db).await?;
    //         assert_eq!(
    //             chain.get_persisted_cap_claim_by_secret(&secret).await?,
    //             Some(claim)
    //         );
    //     }
    //
    //     Ok(())

    #[tokio::test(flavor = "multi_thread")]
    async fn source_chain_buffer_iter_back() -> SourceChainResult<()> {
        observability::test_run().ok();
        let test_db = test_authored_db();
        let dht_db = test_dht_db();
        let dht_db_cache = DhtDbQueryCache::new(dht_db.to_db().into());
        let keystore = test_keystore();
        let vault = test_db.to_db();
        let mut mock = MockHolochainP2pDnaT::new();
        mock.expect_authority_for_hash().returning(|_| Ok(false));

        let author = Arc::new(keystore.new_sign_keypair_random().await.unwrap());

        fresh_reader_test!(vault, |txn| {
            assert_matches!(
                chain_head_db(&txn, author.clone()),
                Err(SourceChainError::ChainEmpty)
            );
        });
        genesis(
            vault.clone().into(),
            dht_db.to_db(),
            &dht_db_cache,
            keystore.clone(),
            fixt!(DnaHash),
            (*author).clone(),
            None,
        )
        .await
        .unwrap();

        let source_chain = SourceChain::new(
            vault.clone().into(),
            dht_db.to_db(),
            dht_db_cache.clone(),
            keystore.clone(),
            (*author).clone(),
        )
        .await
        .unwrap();
        let entry = Entry::App(fixt!(AppEntryBytes));
        let create = builder::Create {
            entry_type: EntryType::App(fixt!(AppEntryType)),
            entry_hash: EntryHash::with_data_sync(&entry),
        };
        let h1 = source_chain
            .put_weightless(create, Some(entry), ChainTopOrdering::default())
            .await
            .unwrap();
        let entry = Entry::App(fixt!(AppEntryBytes));
        let create = builder::Create {
            entry_type: EntryType::App(fixt!(AppEntryType)),
            entry_hash: EntryHash::with_data_sync(&entry),
        };
        let h2 = source_chain
            .put_weightless(create, Some(entry), ChainTopOrdering::default())
            .await
            .unwrap();
        source_chain.flush(&mock).await.unwrap();

        fresh_reader_test!(vault, |txn| {
            assert_eq!(chain_head_db(&txn, author.clone()).unwrap().0, h2);
            // get the full record
            let store = Txn::from(&txn);
            let h1_record_fetched = store
                .get_record(&h1.clone().into())
                .expect("error retrieving")
                .expect("entry not found");
            let h2_record_fetched = store
                .get_record(&h2.clone().into())
                .expect("error retrieving")
                .expect("entry not found");
            assert_eq!(h1, *h1_record_fetched.action_address());
            assert_eq!(h2, *h2_record_fetched.action_address());
        });

        // check that you can iterate on the chain
        let source_chain = SourceChain::new(
            vault.clone(),
            dht_db.to_db(),
            dht_db_cache.clone(),
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
        let test_db = test_authored_db();
        let dht_db = test_dht_db();
        let dht_db_cache = DhtDbQueryCache::new(dht_db.to_db().into());
        let keystore = test_keystore();
        let vault = test_db.to_db();
        let author = keystore.new_sign_keypair_random().await.unwrap();
        genesis(
            vault.clone().into(),
            dht_db.to_db(),
            &dht_db_cache,
            keystore.clone(),
            fixt!(DnaHash),
            author.clone(),
            None,
        )
        .await
        .unwrap();

        let json = dump_state(vault.clone().into(), author.clone()).await?;
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
        let test_db = test_authored_db();
        let dht_db = test_dht_db();
        let dht_db_cache = DhtDbQueryCache::new(dht_db.to_db().into());
        let keystore = test_keystore();
        let vault = test_db.to_db();
        let alice = keystore.new_sign_keypair_random().await.unwrap();
        let bob = keystore.new_sign_keypair_random().await.unwrap();
        let dna_hash = fixt!(DnaHash);

        genesis(
            vault.clone().into(),
            dht_db.to_db(),
            &dht_db_cache,
            keystore.clone(),
            dna_hash.clone(),
            alice.clone(),
            None,
        )
        .await
        .unwrap();

        genesis(
            vault.clone().into(),
            dht_db.to_db(),
            &dht_db_cache,
            keystore.clone(),
            dna_hash.clone(),
            bob.clone(),
            None,
        )
        .await
        .unwrap();

        test_db.dump_tmp();

        let chain = SourceChain::new(vault, dht_db.to_db(), dht_db_cache, keystore, alice.clone())
            .await
            .unwrap();

        let elements = chain.query(ChainQueryFilter::default()).await.unwrap();

        // All of the range queries which should return a full set of elements
        let full_ranges = [
            ChainQueryFilterRange::Unbounded,
            ChainQueryFilterRange::ActionSeqRange(0, 2),
            ChainQueryFilterRange::ActionHashRange(
                elements[0].action_address().clone(),
                elements[2].action_address().clone(),
            ),
            ChainQueryFilterRange::ActionHashTerminated(elements[2].action_address().clone(), 2),
        ];

        // A variety of combinations of query parameters
        let cases = [
            ((None, None, vec![], false), 3),
            ((None, None, vec![], true), 3),
            ((Some(ActionType::Dna), None, vec![], false), 1),
            ((None, Some(EntryType::AgentPubKey), vec![], false), 1),
            ((None, Some(EntryType::AgentPubKey), vec![], true), 1),
            ((Some(ActionType::Create), None, vec![], false), 1),
            ((Some(ActionType::Create), None, vec![], true), 1),
            (
                (
                    Some(ActionType::Create),
                    Some(EntryType::AgentPubKey),
                    vec![],
                    false,
                ),
                1,
            ),
            (
                (
                    Some(ActionType::Create),
                    Some(EntryType::AgentPubKey),
                    vec![elements[2].action().entry_hash().unwrap().clone()],
                    true,
                ),
                1,
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
                    descending: false,
                };
                if sequence_range != ChainQueryFilterRange::Unbounded
                    && (action_type.is_some()
                        || entry_type.is_some()
                        || entry_hashes.is_some()
                        || include_entries)
                {
                    assert!(matches!(
                        chain.query(query.clone()).await,
                        Err(SourceChainError::UnsupportedQuery(_))
                    ));
                } else {
                    let queried = chain.query(query.clone()).await.unwrap();
                    let actual = queried.len();
                    assert!(queried.iter().all(|e| e.action().author() == &alice));
                    assert_eq!(
                        num_expected, actual,
                        "Expected {} items but got {} with filter {:?}",
                        num_expected, actual, query
                    );
                }
            }
        }
    }
}
