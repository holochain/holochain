use crate::prelude::*;
use holo_hash::ActionHash;
use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holochain_keystore::KeystoreError;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use thiserror::Error;

// The scratch stages v2 `Action`/`SignedActionHashed`/`Record`, matching what
// `source_chain.rs` builds during authoring. These explicit imports pin the
// v2 shape (rather than relying on `crate::prelude::*`, which resolves
// `Action`/`Record`/`SignedActionHashed` to the legacy shape for the benefit
// of the legacy op-pipeline and its SQL query machinery in `query.rs`).
use holochain_zome_types::dht_v2::to_legacy_signed_action;
use holochain_zome_types::record::Record;
use holochain_zome_types::record::SignedActionHashed;
// The `Store`/`Stores`/`StoresIter` traits (`query.rs`) belong to the legacy
// SQL query machinery and are typed over the legacy `SignedActionHashed`/
// `Record`. `Scratch`'s implementations of those traits convert its v2
// actions to legacy at the boundary with these aliases.
use holochain_zome_types::dependencies::holochain_integrity_types::record::Record as LegacyRecord;
use holochain_zome_types::dependencies::holochain_integrity_types::record::SignedActionHashed as LegacySignedActionHashed;

/// The "scratch" is an in-memory space to stage Actions to be committed at the
/// end of the CallZome workflow.
///
/// This space must also be queryable: specifically, it needs to be combined
/// into queries into the database which return Actions. This is done by
/// a simple filter on the scratch space, and then chaining that iterator
/// onto the iterators over the Actions in the database(s) produced by the
/// Cascade.
#[derive(Debug, Clone, Default)]
pub struct Scratch {
    actions: Vec<SignedActionHashed>,
    entries: HashMap<EntryHash, Arc<Entry>>,
    chain_top_ordering: ChainTopOrdering,
    scheduled_fns: Vec<ScheduledFn>,
    chain_head: Option<(u32, usize)>,
    warrants: Vec<SignedWarrant>,
}

#[derive(Debug, Clone)]
pub struct SyncScratch(Arc<Mutex<Scratch>>);

// MD: hmm, why does this need to be a separate type? Why collect into this?
//
// Holds legacy actions — it feeds `query.rs`'s legacy SQL query machinery
// (via `Stores`/`StoresIter`), so `Scratch::as_filter` converts each v2
// scratch action to legacy when building this.
pub struct FilteredScratch {
    actions: Vec<LegacySignedActionHashed>,
}

impl Scratch {
    pub fn new() -> Self {
        Self {
            chain_top_ordering: ChainTopOrdering::Relaxed,
            ..Default::default()
        }
    }

    pub fn scheduled_fns(&self) -> &[ScheduledFn] {
        &self.scheduled_fns
    }

    pub fn add_scheduled_fn(&mut self, scheduled_fn: ScheduledFn) {
        self.scheduled_fns.push(scheduled_fn)
    }

    pub fn chain_top_ordering(&self) -> ChainTopOrdering {
        self.chain_top_ordering
    }

    pub fn respect_chain_top_ordering(&mut self, chain_top_ordering: ChainTopOrdering) {
        if chain_top_ordering == ChainTopOrdering::Strict {
            self.chain_top_ordering = chain_top_ordering;
        }
    }

    pub fn add_action(&mut self, item: SignedActionHashed, chain_top_ordering: ChainTopOrdering) {
        self.respect_chain_top_ordering(chain_top_ordering);
        let seq = item.action().action_seq();
        match &mut self.chain_head {
            Some((h, i)) => {
                if seq > *h {
                    *h = seq;
                    *i = self.actions.len();
                }
            }
            h @ None => *h = Some((seq, self.actions.len())),
        }
        self.actions.push(item);
    }

    pub fn chain_head(&self) -> Option<HeadInfo> {
        self.chain_head.as_ref().and_then(|(_, i)| {
            self.actions.get(*i).map(|h| HeadInfo {
                action: h.action_address().clone(),
                seq: h.action().action_seq(),
                timestamp: h.action().timestamp(),
            })
        })
    }

    pub fn add_entry(&mut self, entry_hashed: EntryHashed, chain_top_ordering: ChainTopOrdering) {
        self.respect_chain_top_ordering(chain_top_ordering);
        let (entry, hash) = entry_hashed.into_inner();
        self.entries.insert(hash, Arc::new(entry));
    }

    /// Filter the scratch's actions for `query.rs`'s legacy SQL query
    /// machinery, converting each matching v2 action to legacy.
    pub fn as_filter(&self, f: impl Fn(&LegacySignedActionHashed) -> bool) -> FilteredScratch {
        let actions = self
            .actions
            .iter()
            .map(to_legacy_signed_action)
            .filter(f)
            .collect();
        FilteredScratch { actions }
    }

    pub fn into_sync(self) -> SyncScratch {
        SyncScratch(Arc::new(Mutex::new(self)))
    }

    pub fn len(&self) -> usize {
        self.actions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty() && self.scheduled_fns.is_empty() && self.warrants.is_empty()
    }

    pub fn actions(&self) -> impl Iterator<Item = &SignedActionHashed> {
        self.actions.iter()
    }

    pub fn records(&self) -> impl Iterator<Item = Record> + '_ {
        self.actions.iter().cloned().map(move |shh| {
            let entry = shh
                .action()
                .entry_hash()
                // TODO: let's use Arc<Entry> from here on instead of dereferencing
                .and_then(|eh| self.entries.get(eh).map(|e| (**e).clone()));
            let record_entry = RecordEntry::new(shh.action().entry_visibility(), entry);
            Record::new(shh, record_entry)
        })
    }

    /// Get the entries on in the scratch.
    pub fn entries(&self) -> impl Iterator<Item = (&EntryHash, &Arc<Entry>)> {
        self.entries.iter()
    }

    fn get_exact_record(&self, hash: &ActionHash) -> StateQueryResult<Option<Record>> {
        // `Store::get_action` (`query.rs`) belongs to the legacy SQL query
        // machinery and returns a legacy action, so this searches the (v2)
        // `actions()` directly instead of calling it.
        let Some(shh) = self.actions().find(|h| h.action_address() == hash).cloned() else {
            return Ok(None);
        };
        let entry = shh
            .action()
            .entry_hash()
            .and_then(|eh| self.get_entry(eh).ok())
            .flatten();
        let record_entry = RecordEntry::new(shh.action().entry_visibility(), entry);
        Ok(Some(Record::new(shh, record_entry)))
    }

    fn get_any_record(&self, hash: &EntryHash) -> StateQueryResult<Option<Record>> {
        let r = self.get_entry(hash)?.and_then(|entry| {
            let shh = self
                .actions()
                .find(|&h| {
                    h.action()
                        .entry_hash()
                        .map(|eh| eh == hash)
                        .unwrap_or(false)
                })?
                .clone();
            let record_entry = RecordEntry::new(shh.action().entry_visibility(), Some(entry));
            Some(Record::new(shh, record_entry))
        });
        Ok(r)
    }

    pub fn drain_scheduled_fns(&mut self) -> impl Iterator<Item = ScheduledFn> + '_ {
        self.scheduled_fns.drain(..)
    }

    /// Drain out all the actions.
    pub fn drain_actions(&mut self) -> impl Iterator<Item = SignedActionHashed> + '_ {
        self.chain_head = None;
        self.actions.drain(..)
    }

    /// Drain out all the entries.
    pub fn drain_entries(&mut self) -> impl Iterator<Item = EntryHashed> + '_ {
        self.entries.drain().map(|(hash, entry)| {
            EntryHashed::with_pre_hashed(
                Arc::try_unwrap(entry).unwrap_or_else(|e| (*e).clone()),
                hash,
            )
        })
    }

    pub fn add_warrant(&mut self, warrant: SignedWarrant) {
        self.warrants.push(warrant);
    }

    pub fn warrants(&self) -> impl Iterator<Item = &SignedWarrant> {
        self.warrants.iter()
    }

    pub fn drain_warrants(&mut self) -> impl Iterator<Item = SignedWarrant> + '_ {
        self.warrants.drain(..)
    }
}

impl SyncScratch {
    pub fn apply<T, F: FnOnce(&mut Scratch) -> T>(&self, f: F) -> Result<T, SyncScratchError> {
        Ok(f(&mut *self
            .0
            .lock()
            .map_err(|_| SyncScratchError::ScratchLockPoison)?))
    }

    pub fn apply_and_then<T, E, F>(&self, f: F) -> Result<T, E>
    where
        E: From<SyncScratchError>,
        F: FnOnce(&mut Scratch) -> Result<T, E>,
    {
        f(&mut *self
            .0
            .lock()
            .map_err(|_| SyncScratchError::ScratchLockPoison)?)
    }
}

// `Store` (`query.rs`) belongs to the legacy SQL query machinery and is typed
// over the legacy `SignedActionHashed`/`Record`; every method here converts
// the (v2) scratch action to legacy at the return boundary.
impl Store for Scratch {
    fn get_entry(&self, hash: &EntryHash) -> StateQueryResult<Option<Entry>> {
        Ok(self.entries.get(hash).map(|arc| (**arc).clone()))
    }

    fn contains_entry(&self, hash: &EntryHash) -> StateQueryResult<bool> {
        Ok(self.entries.contains_key(hash))
    }

    fn contains_action(&self, hash: &ActionHash) -> StateQueryResult<bool> {
        Ok(self.actions().any(|h| h.action_address() == hash))
    }

    fn get_action(&self, hash: &ActionHash) -> StateQueryResult<Option<LegacySignedActionHashed>> {
        Ok(self
            .actions()
            .find(|&h| h.action_address() == hash)
            .map(to_legacy_signed_action))
    }

    fn get_warrants_for_agent(
        &self,
        agent_key: &AgentPubKey,
        _check_validity: bool,
    ) -> StateQueryResult<Vec<WarrantOp>> {
        Ok(self
            .warrants
            .iter()
            .filter_map(|warrant| {
                // Check if this warrant applies to the agent
                if warrant.warrantee == *agent_key {
                    Some(WarrantOp::from(warrant.clone()))
                } else {
                    None
                }
            })
            .collect())
    }

    fn get_record(&self, hash: &AnyDhtHash) -> StateQueryResult<Option<LegacyRecord>> {
        let record = match hash.clone().into_primitive() {
            AnyDhtHashPrimitive::Entry(hash) => self.get_any_record(&hash),
            AnyDhtHashPrimitive::Action(hash) => self.get_exact_record(&hash),
        }?;
        Ok(record.map(|record| {
            let (signed_action, entry) = record.into_inner();
            LegacyRecord {
                signed_action: to_legacy_signed_action(&signed_action),
                entry,
            }
        }))
    }

    fn get_public_record(&self, hash: &AnyDhtHash) -> StateQueryResult<Option<LegacyRecord>> {
        self.get_record(hash)
    }

    /// It doesn't make sense to search for
    /// a different authored entry in a scratch
    /// then the scratches author so this is
    /// the same as `get_entry`.
    fn get_public_or_authored_entry(
        &self,
        hash: &EntryHash,
        _author: Option<&AgentPubKey>,
    ) -> StateQueryResult<Option<Entry>> {
        self.get_entry(hash)
    }

    /// It doesn't make sense to search for
    /// a different authored record in a scratch
    /// then the scratches author so this is
    /// the same as `get_record`.
    fn get_public_or_authored_record(
        &self,
        hash: &AnyDhtHash,
        _author: Option<&AgentPubKey>,
    ) -> StateQueryResult<Option<LegacyRecord>> {
        self.get_record(hash)
    }
}

impl FilteredScratch {
    pub fn drain(&mut self) -> impl Iterator<Item = LegacySignedActionHashed> + '_ {
        self.actions.drain(..)
    }
}

impl<Q> Stores<Q> for Scratch
where
    Q: Query<Item = Judged<LegacySignedActionHashed>>,
{
    type O = FilteredScratch;

    fn get_initial_data(&self, query: Q) -> StateQueryResult<Self::O> {
        Ok(self.as_filter(query.as_filter()))
    }
}

impl StoresIter<Judged<LegacySignedActionHashed>> for FilteredScratch {
    fn iter(&mut self) -> StateQueryResult<StmtIter<'_, Judged<LegacySignedActionHashed>>> {
        // We are assuming data in the scratch space is valid even though
        // it hasn't been validated yet because if it does fail validation
        // then this transaction will be rolled back.
        // TODO: Write test to prove this assumption.
        Ok(Box::new(fallible_iterator::convert(
            self.drain().map(Judged::valid).map(Ok),
        )))
    }
}

#[derive(Error, Debug)]
pub enum ScratchError {
    #[error(transparent)]
    Timestamp(#[from] TimestampError),

    #[error(transparent)]
    Keystore(#[from] KeystoreError),

    #[error(transparent)]
    Action(#[from] ActionError),

    /// Other
    #[error("Other: {0}")]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl ScratchError {
    /// promote a custom error type to a ScratchError
    pub fn other(e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::Other(e.into())
    }
}

impl From<one_err::OneErr> for ScratchError {
    fn from(e: one_err::OneErr) -> Self {
        Self::other(e)
    }
}

#[derive(Error, Debug)]
pub enum SyncScratchError {
    #[error("Scratch lock was poisoned")]
    ScratchLockPoison,
}
