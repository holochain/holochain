use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use holo_hash::hash_type::AnyDht;
use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_keystore::KeystoreError;
use holochain_types::prelude::*;
use holochain_zome_types::entry::EntryHashed;
use holochain_zome_types::ChainTopOrdering;
use holochain_zome_types::Element;
use holochain_zome_types::Entry;
use holochain_zome_types::SignedHeaderHashed;
use holochain_zome_types::TimestampError;
use thiserror::Error;

use crate::prelude::Query;
use crate::prelude::Stores;
use crate::prelude::StoresIter;
use crate::query::StateQueryResult;
use crate::query::StmtIter;
use crate::query::Store;

/// The "scratch" is an in-memory space to stage Headers to be committed at the
/// end of the CallZome workflow.
///
/// This space must also be queryable: specifically, it needs to be combined
/// into queries into the database which return Headers. This is done by
/// a simple filter on the scratch space, and then chaining that iterator
/// onto the iterators over the Headers in the database(s) produced by the
/// Cascade.
#[derive(Debug, Clone, Default)]
pub struct Scratch {
    headers: Vec<SignedHeaderHashed>,
    entries: HashMap<EntryHash, Arc<Entry>>,
    chain_top_ordering: ChainTopOrdering,
}

#[derive(Debug, Clone)]
pub struct SyncScratch(Arc<Mutex<Scratch>>);

// MD: hmm, why does this need to be a separate type? Why collect into this?
pub struct FilteredScratch {
    headers: Vec<SignedHeaderHashed>,
}

impl Scratch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn chain_top_ordering(&self) -> ChainTopOrdering {
        self.chain_top_ordering
    }

    pub fn respect_chain_top_ordering(&mut self, chain_top_ordering: ChainTopOrdering) {
        if chain_top_ordering == ChainTopOrdering::Strict {
            self.chain_top_ordering = chain_top_ordering;
        }
    }

    pub fn add_header(&mut self, item: SignedHeaderHashed, chain_top_ordering: ChainTopOrdering) {
        self.respect_chain_top_ordering(chain_top_ordering);
        self.headers.push(item);
    }

    pub fn add_entry(&mut self, entry_hashed: EntryHashed, chain_top_ordering: ChainTopOrdering) {
        self.respect_chain_top_ordering(chain_top_ordering);
        let (entry, hash) = entry_hashed.into_inner();
        self.entries.insert(hash, Arc::new(entry));
    }

    pub fn as_filter(&self, f: impl Fn(&SignedHeaderHashed) -> bool) -> FilteredScratch {
        let headers = self.headers.iter().filter(|&t| f(t)).cloned().collect();
        FilteredScratch { headers }
    }

    pub fn into_sync(self) -> SyncScratch {
        SyncScratch(Arc::new(Mutex::new(self)))
    }

    pub fn len(&self) -> usize {
        self.headers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.headers.is_empty()
    }

    pub fn headers(&self) -> impl Iterator<Item = &SignedHeaderHashed> {
        self.headers.iter()
    }

    pub fn elements(&self) -> impl Iterator<Item = Element> + '_ {
        self.headers.iter().cloned().map(move |shh| {
            let entry = shh
                .header()
                .entry_hash()
                .and_then(|eh| self.entries.get(eh).map(|e| (**e).clone()));
            Element::new(shh, entry)
        })
    }

    /// Get the entries on in the scratch.
    pub fn entries(&self) -> impl Iterator<Item = (&EntryHash, &Arc<Entry>)> {
        self.entries.iter()
    }

    pub fn num_headers(&self) -> usize {
        self.headers.len()
    }

    fn get_exact_element(
        &self,
        hash: &HeaderHash,
    ) -> StateQueryResult<Option<holochain_zome_types::Element>> {
        Ok(self.get_header(hash)?.map(|shh| {
            let entry = shh
                .header()
                .entry_hash()
                .and_then(|eh| self.get_entry(eh).ok());
            Element::new(shh, entry.flatten())
        }))
    }

    fn get_any_element(
        &self,
        hash: &EntryHash,
    ) -> StateQueryResult<Option<holochain_zome_types::Element>> {
        let r = self.get_entry(hash)?.and_then(|entry| {
            let shh = self
                .headers
                .iter()
                .find(|h| {
                    h.header()
                        .entry_hash()
                        .map(|eh| eh == hash)
                        .unwrap_or(false)
                })?
                .clone();
            Some(Element::new(shh, Some(entry)))
        });
        Ok(r)
    }

    /// Drain out all the headers.
    pub fn drain_headers(&mut self) -> impl Iterator<Item = SignedHeaderHashed> + '_ {
        self.headers.drain(..)
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

impl Store for Scratch {
    fn get_entry(&self, hash: &EntryHash) -> StateQueryResult<Option<Entry>> {
        Ok(self.entries.get(hash).map(|arc| (**arc).clone()))
    }

    fn contains_entry(&self, hash: &EntryHash) -> StateQueryResult<bool> {
        Ok(self.entries.contains_key(hash))
    }

    fn contains_header(&self, hash: &HeaderHash) -> StateQueryResult<bool> {
        Ok(self.headers.iter().any(|h| h.header_address() == hash))
    }

    fn get_header(&self, hash: &HeaderHash) -> StateQueryResult<Option<SignedHeaderHashed>> {
        Ok(self
            .headers
            .iter()
            .find(|h| h.header_address() == hash)
            .cloned())
    }

    fn get_element(&self, hash: &AnyDhtHash) -> StateQueryResult<Option<Element>> {
        match *hash.hash_type() {
            AnyDht::Entry => self.get_any_element(&hash.clone().into()),
            AnyDht::Header => self.get_exact_element(&hash.clone().into()),
        }
    }
}

impl FilteredScratch {
    pub fn drain(&mut self) -> impl Iterator<Item = SignedHeaderHashed> + '_ {
        self.headers.drain(..)
    }
}

impl<Q> Stores<Q> for Scratch
where
    Q: Query<Item = Judged<SignedHeaderHashed>>,
{
    type O = FilteredScratch;

    fn get_initial_data(&self, query: Q) -> StateQueryResult<Self::O> {
        Ok(self.as_filter(query.as_filter()))
    }
}

impl StoresIter<Judged<SignedHeaderHashed>> for FilteredScratch {
    fn iter(&mut self) -> StateQueryResult<StmtIter<'_, Judged<SignedHeaderHashed>>> {
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
    Header(#[from] HeaderError),
}

#[derive(Error, Debug)]
pub enum SyncScratchError {
    #[error("Scratch lock was poisoned")]
    ScratchLockPoison,
}

#[test]
fn test_multiple_in_memory() {
    use holochain_sqlite::rusqlite::*;

    // blank string means "temporary database", which typically resides in
    // memory but can be flushed to disk if sqlite is under memory pressure
    let mut m1 = Connection::open("").unwrap();
    let mut m2 = Connection::open("").unwrap();

    let schema = "
CREATE TABLE mytable (
    x INTEGER PRIMARY KEY
);
    ";

    m1.execute(schema, NO_PARAMS).unwrap();
    m2.execute(schema, NO_PARAMS).unwrap();

    let num = m1
        .execute("INSERT INTO mytable (x) VALUES (1)", NO_PARAMS)
        .unwrap();
    assert_eq!(num, 1);

    let xs1: Vec<u16> = m1
        .transaction()
        .unwrap()
        .prepare_cached("SELECT x FROM mytable")
        .unwrap()
        .query_map(NO_PARAMS, |row| row.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    let xs2: Vec<u16> = m2
        .transaction()
        .unwrap()
        .prepare_cached("SELECT * FROM mytable")
        .unwrap()
        .query_map(NO_PARAMS, |row| row.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(xs1, vec![1]);
    assert!(xs2.is_empty());
}
