use std::collections::HashMap;
use std::sync::Arc;

use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_types::prelude::ValStatusOf;
use holochain_types::EntryHashed;
use holochain_zome_types::Entry;
use holochain_zome_types::SignedHeaderHashed;

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
#[derive(Debug, Clone)]
pub struct Scratch {
    headers: Vec<SignedHeaderHashed>,
    entries: HashMap<EntryHash, Arc<Entry>>,
}

// MD: hmm, why does this need to be a separate type? Why collect into this?
pub struct FilteredScratch {
    headers: Vec<SignedHeaderHashed>,
}

impl Scratch {
    pub fn new() -> Self {
        Self {
            headers: Vec::new(),
            entries: HashMap::new(),
        }
    }

    pub fn add_header(&mut self, item: SignedHeaderHashed) {
        self.headers.push(item);
    }

    pub fn add_entry(&mut self, entry_hashed: EntryHashed) {
        let (entry, hash) = entry_hashed.into_inner();
        self.entries.insert(hash, Arc::new(entry));
    }

    pub fn as_filter(&self, f: impl Fn(&SignedHeaderHashed) -> bool) -> FilteredScratch {
        let headers = self.headers.iter().filter(|&t| f(t)).cloned().collect();
        FilteredScratch { headers }
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
        Ok(self
            .headers
            .iter()
            .find(|h| h.header_address() == hash)
            .is_some())
    }
}

impl FilteredScratch {
    pub fn into_iter<'iter>(&'iter mut self) -> impl Iterator<Item = SignedHeaderHashed> + 'iter {
        self.headers.drain(..)
    }
}

impl<Q> Stores<Q> for Scratch
where
    Q: Query<Data = SignedHeaderHashed, ValidatedData = ValStatusOf<SignedHeaderHashed>>,
{
    type O = FilteredScratch;

    fn get_initial_data(&self, query: Q) -> StateQueryResult<Self::O> {
        Ok(self.as_filter(query.as_filter()))
    }
}

impl StoresIter<ValStatusOf<SignedHeaderHashed>> for FilteredScratch {
    fn iter(&mut self) -> StateQueryResult<StmtIter<'_, ValStatusOf<SignedHeaderHashed>>> {
        // We are assuming data in the scratch space is valid even though
        // it hasn't been validated yet because if it does fail validation
        // then this transaction will be rolled back.
        // TODO: Write test to prove this assumption.
        Ok(Box::new(fallible_iterator::convert(
            self.into_iter().map(ValStatusOf::valid).map(Ok),
        )))
    }
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
        .query_map(NO_PARAMS, |row| Ok(dbg!(row.get(0))?))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    let xs2: Vec<u16> = m2
        .transaction()
        .unwrap()
        .prepare_cached("SELECT * FROM mytable")
        .unwrap()
        .query_map(NO_PARAMS, |row| Ok(row.get(0)?))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(xs1, vec![1]);
    assert!(xs2.is_empty());
}
