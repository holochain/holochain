use crate::scratch::FilteredScratch;
use crate::scratch::Scratch;
use fallible_iterator::FallibleIterator;
use holo_hash::hash_type::AnyDht;
use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_serialized_bytes::prelude::*;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::Row;
use holochain_sqlite::rusqlite::Statement;
use holochain_sqlite::rusqlite::Transaction;
use holochain_zome_types::Entry;
use holochain_zome_types::HeaderHashed;
use holochain_zome_types::SignedHeader;
use holochain_zome_types::SignedHeaderHashed;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

pub use error::*;

#[cfg(test)]
mod test_data;
#[cfg(test)]
mod tests;

pub mod chain_head;
pub mod error;
pub mod link;
pub mod live_element;
pub mod live_entry;

pub mod prelude {
    pub use super::from_blob;
    pub use super::get_entry_from_db;
    pub use super::to_blob;
    pub use super::Params;
    pub use super::Query;
    pub use super::StateQueryResult;
    pub use super::Store;
    pub use super::Stores;
    pub use super::StoresIter;
    pub use super::Transactions;
    pub use super::Txn;
    pub use super::Txns;
    pub use holochain_sqlite::rusqlite::named_params;
    pub use holochain_sqlite::rusqlite::Row;
    pub use std::sync::Arc;
}

/// Alias for the params required by rusqlite query execution
pub type Params<'a> = (&'a str, &'a dyn holochain_sqlite::rusqlite::ToSql);

/// A common accumulator type used by folds to collapse queries down to a
/// simpler structure, i.e. to let deletions annihilate creations.
pub struct Maps<T> {
    pub creates: HashMap<HeaderHash, T>,
    pub deletes: HashSet<HeaderHash>,
}

impl<T> Maps<T> {
    fn new() -> Self {
        Self {
            creates: Default::default(),
            deletes: Default::default(),
        }
    }
}

/// You should keep your query type cheap to clone.
/// If there is any large data put it in an Arc.
pub trait Query: Clone {
    type State;
    type Data: Clone;
    type Output;

    fn query(&self) -> &str {
        ""
    }
    fn params(&self) -> Vec<Params> {
        Vec::with_capacity(0)
    }
    fn init_fold(&self) -> StateQueryResult<Self::State>;

    fn as_filter(&self) -> Box<dyn Fn(&Self::Data) -> bool> {
        Box::new(|_| true)
    }

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Data>>;

    fn fold(&self, state: Self::State, data: Self::Data) -> StateQueryResult<Self::State>;

    fn run<S>(&self, stores: S) -> StateQueryResult<Self::Output>
    where
        S: Stores<Self>,
        S: Store,
    {
        let mut stores_iter = stores.get_initial_data(self.clone())?;
        let iter = stores_iter.iter()?;
        let result = iter.fold(self.init_fold()?, |state, i| self.fold(state, i))?;
        drop(stores_iter);
        self.render(result, stores)
    }

    fn render<S>(&self, state: Self::State, stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store;
}

/// Represents the data sources which are needed to perform a Query.
/// From these sources, we need:
/// - a collection of Data needed by the query (`Q::Data`)
/// - the ability to fetch an Entry during the Render phase of the query.
pub trait Stores<Q: Query> {
    type O: StoresIter<Q::Data>;

    /// Gets the raw initial data from the database, needed to begin the query.
    // MD: can the query be &Q?
    fn get_initial_data(&self, query: Q) -> StateQueryResult<Self::O>;
}

pub trait Store {
    /// Get an Entry from the database
    fn get_entry(&self, hash: &EntryHash) -> StateQueryResult<Option<Entry>>;

    /// Check if a hash is contained in the store
    fn contains_hash(&self, hash: &AnyDhtHash) -> StateQueryResult<bool> {
        match *hash.hash_type() {
            AnyDht::Entry => self.contains_entry(&hash.clone().into()),
            AnyDht::Header => self.contains_header(&hash.clone().into()),
        }
    }

    /// Check if an entry is contained in the store
    fn contains_entry(&self, hash: &EntryHash) -> StateQueryResult<bool>;

    /// Check if a header is contained in the store
    fn contains_header(&self, hash: &HeaderHash) -> StateQueryResult<bool>;
}

/// Each Stores implementation has its own custom way of iterating over itself,
/// which this trait represents.
// MD: does this definitely need to be its own trait? Why can't a Stores
// just return an iterator?
pub trait StoresIter<T> {
    fn iter<'iter>(&'iter mut self) -> StateQueryResult<StmtIter<'iter, T>>;
}

/// Wrapper around a transaction reference, to which trait impls are attached
pub struct Txn<'borrow, 'txn> {
    txn: &'borrow Transaction<'txn>,
}

/// Wrapper around a collection of Txns, to which trait impls are attached
pub struct Txns<'borrow, 'txn> {
    txns: Vec<Txn<'borrow, 'txn>>,
}

/// Alias for an array of Transaction references
pub type Transactions<'a, 'txn> = [&'a Transaction<'txn>];

pub struct DbScratch<'borrow, 'txn> {
    txns: Txns<'borrow, 'txn>,
    scratch: &'borrow Scratch,
}

pub struct DbScratchIter<'stmt, Q>
where
    Q: Query<Data = SignedHeaderHashed>,
{
    stmts: QueryStmts<'stmt, Q>,
    filtered_scratch: FilteredScratch,
}

impl<'stmt, Q: Query> Stores<Q> for Txn<'stmt, '_> {
    type O = QueryStmt<'stmt, Q>;

    fn get_initial_data(&self, query: Q) -> StateQueryResult<Self::O> {
        QueryStmt::new(&self.txn, query)
    }
}

impl<'stmt> Store for Txn<'stmt, '_> {
    fn get_entry(&self, hash: &EntryHash) -> StateQueryResult<Option<Entry>> {
        get_entry_from_db(&self.txn, hash)
    }

    fn contains_entry(&self, _hash: &EntryHash) -> StateQueryResult<bool> {
        todo!()
    }

    fn contains_header(&self, _hash: &HeaderHash) -> StateQueryResult<bool> {
        todo!()
    }
}

impl<'stmt, Q: Query> StoresIter<Q::Data> for QueryStmt<'stmt, Q> {
    fn iter(&mut self) -> StateQueryResult<StmtIter<'_, Q::Data>> {
        self.iter()
    }
}

impl<'stmt, Q: Query> Stores<Q> for Txns<'stmt, '_> {
    type O = QueryStmts<'stmt, Q>;

    fn get_initial_data(&self, query: Q) -> StateQueryResult<Self::O> {
        let stmts = fallible_iterator::convert(
            self.txns
                .iter()
                .map(|txn| txn.get_initial_data(query.clone())),
        )
        .collect()?;
        Ok(QueryStmts { stmts })
    }
}

impl<'stmt> Store for Txns<'stmt, '_> {
    fn get_entry(&self, hash: &EntryHash) -> StateQueryResult<Option<Entry>> {
        for txn in &self.txns {
            let r = txn.get_entry(hash)?;
            if r.is_some() {
                return Ok(r);
            }
        }
        Ok(None)
    }

    fn contains_entry(&self, hash: &EntryHash) -> StateQueryResult<bool> {
        for txn in &self.txns {
            let r = txn.contains_entry(hash)?;
            if r {
                return Ok(r);
            }
        }
        Ok(false)
    }

    fn contains_header(&self, hash: &HeaderHash) -> StateQueryResult<bool> {
        for txn in &self.txns {
            let r = txn.contains_header(hash)?;
            if r {
                return Ok(r);
            }
        }
        Ok(false)
    }
}

impl<'stmt, Q: Query> StoresIter<Q::Data> for QueryStmts<'stmt, Q> {
    fn iter(&mut self) -> StateQueryResult<StmtIter<'_, Q::Data>> {
        Ok(Box::new(
            fallible_iterator::convert(self.stmts.iter_mut().map(Ok)).flat_map(|stmt| stmt.iter()),
        ))
    }
}

impl<Q> Stores<Q> for Scratch
where
    Q: Query<Data = SignedHeaderHashed>,
{
    type O = FilteredScratch;

    fn get_initial_data(&self, query: Q) -> StateQueryResult<Self::O> {
        Ok(self.as_filter(query.as_filter()))
    }
}

impl StoresIter<SignedHeaderHashed> for FilteredScratch {
    fn iter(&mut self) -> StateQueryResult<StmtIter<'_, SignedHeaderHashed>> {
        Ok(Box::new(fallible_iterator::convert(
            self.into_iter().map(Ok),
        )))
    }
}

impl<'borrow, 'txn, Q> Stores<Q> for DbScratch<'borrow, 'txn>
where
    Q: Query<Data = SignedHeaderHashed>,
{
    type O = DbScratchIter<'borrow, Q>;

    fn get_initial_data(&self, query: Q) -> StateQueryResult<Self::O> {
        Ok(DbScratchIter {
            stmts: self.txns.get_initial_data(query.clone())?,
            filtered_scratch: self.scratch.get_initial_data(query.clone())?,
        })
    }
}

impl<'borrow, 'txn> Store for DbScratch<'borrow, 'txn> {
    fn get_entry(&self, hash: &EntryHash) -> StateQueryResult<Option<Entry>> {
        let r = self.txns.get_entry(hash)?;
        if r.is_none() {
            self.scratch.get_entry(hash)
        } else {
            Ok(r)
        }
    }

    fn contains_entry(&self, hash: &EntryHash) -> StateQueryResult<bool> {
        let r = self.txns.contains_entry(hash)?;
        if !r {
            self.scratch.contains_entry(hash)
        } else {
            Ok(r)
        }
    }

    fn contains_header(&self, hash: &HeaderHash) -> StateQueryResult<bool> {
        let r = self.txns.contains_header(hash)?;
        if !r {
            self.scratch.contains_header(hash)
        } else {
            Ok(r)
        }
    }
}

impl<'stmt, Q> StoresIter<Q::Data> for DbScratchIter<'stmt, Q>
where
    Q: Query<Data = SignedHeaderHashed>,
{
    fn iter(&mut self) -> StateQueryResult<StmtIter<'_, Q::Data>> {
        Ok(Box::new(
            self.stmts.iter()?.chain(self.filtered_scratch.iter()?),
        ))
    }
}

impl<'borrow, 'txn> DbScratch<'borrow, 'txn> {
    pub fn new(txns: &'borrow Transactions<'borrow, 'txn>, scratch: &'borrow Scratch) -> Self {
        Self {
            txns: txns.into(),
            scratch,
        }
    }
}

impl<'borrow, 'txn> From<&'borrow Transaction<'txn>> for Txn<'borrow, 'txn> {
    fn from(txn: &'borrow Transaction<'txn>) -> Self {
        Self { txn }
    }
}

impl<'borrow, 'txn> From<&'borrow Transactions<'borrow, 'txn>> for Txns<'borrow, 'txn> {
    fn from(txns: &'borrow Transactions<'borrow, 'txn>) -> Self {
        let txns = txns.into_iter().map(|&txn| Txn::from(txn)).collect();
        Self { txns }
    }
}

pub struct QueryStmts<'stmt, Q: Query> {
    stmts: Vec<QueryStmt<'stmt, Q>>,
}

/// A collection of prepared SQL statements used to perform a cascade query
/// on a particular database.
///
/// This type is needed because queries happen in two steps: statement creation,
/// and then statement execution, and a lifetime needs to be enforced across
/// those steps, so we have to hold on to the statements rather than letting
/// them drop as temporary values.
pub struct QueryStmt<'stmt, Q: Query> {
    stmt: Option<Statement<'stmt>>,
    query: Q,
}

type StmtIter<'iter, T> = Box<dyn FallibleIterator<Item = T, Error = StateQueryError> + 'iter>;

impl<'stmt, 'iter, Q: Query> QueryStmt<'stmt, Q> {
    fn new(txn: &'stmt Transaction, query: Q) -> StateQueryResult<Self> {
        let new_stmt = |q: &str| {
            if q.is_empty() {
                Ok(None)
            } else {
                StateQueryResult::Ok(Some(txn.prepare(q)?))
            }
        };
        let stmt = new_stmt(query.query())?;

        Ok(Self { stmt, query })
    }
    fn iter(&'iter mut self) -> StateQueryResult<StmtIter<'iter, Q::Data>> {
        let map_fn = self.query.as_map();
        let iter = Self::new_iter(&self.query.params(), self.stmt.as_mut(), map_fn.clone())?;
        Ok(Box::new(iter))
    }

    fn new_iter<T: 'iter>(
        params: &Vec<Params>,
        stmt: Option<&'iter mut Statement>,
        map_fn: std::sync::Arc<dyn Fn(&Row) -> StateQueryResult<T>>,
    ) -> StateQueryResult<StmtIter<'iter, T>> {
        match stmt {
            Some(stmt) => {
                if params.is_empty() {
                    Ok(Box::new(fallible_iterator::convert(std::iter::empty())) as StmtIter<T>)
                } else {
                    let iter = stmt.query_and_then_named(params, move |r| map_fn(r))?;
                    Ok(Box::new(fallible_iterator::convert(iter)) as StmtIter<T>)
                }
            }
            None => Ok(Box::new(fallible_iterator::convert(std::iter::empty())) as StmtIter<T>),
        }
    }
}

pub fn row_blob_and_hash_to_header(
    blob_index: &'static str,
    hash_index: &'static str,
) -> impl Fn(&Row) -> StateQueryResult<SignedHeaderHashed> {
    move |row| {
        let header = from_blob::<SignedHeader>(row.get(row.column_index(blob_index)?)?);
        let SignedHeader(header, signature) = header;
        let hash: HeaderHash = row.get(row.column_index(hash_index)?)?;
        let header = HeaderHashed::with_pre_hashed(header, hash);
        let shh = SignedHeaderHashed::with_presigned(header, signature);
        Ok(shh)
    }
}

pub fn row_blob_to_header(
    blob_index: &'static str,
) -> impl Fn(&Row) -> StateQueryResult<SignedHeaderHashed> {
    move |row| {
        let header = from_blob::<SignedHeader>(row.get(row.column_index(blob_index)?)?);
        let SignedHeader(header, signature) = header;
        let header = HeaderHashed::from_content_sync(header);
        let shh = SignedHeaderHashed::with_presigned(header, signature);
        Ok(shh)
    }
}

/// Serialize a value to be stored in a database as a BLOB type
pub fn to_blob<T: Serialize + std::fmt::Debug>(t: T) -> Vec<u8> {
    holochain_serialized_bytes::encode(&t).unwrap()
}

/// Deserialize a BLOB from a database into a value
pub fn from_blob<T: DeserializeOwned + std::fmt::Debug>(blob: Vec<u8>) -> T {
    holochain_serialized_bytes::decode(&blob).unwrap()
}

/// Fetch an Entry from a DB by its hash. Requires no joins.
pub fn get_entry_from_db(
    txn: &Transaction,
    entry_hash: &EntryHash,
) -> StateQueryResult<Option<Entry>> {
    let entry = txn.query_row_named(
        "
        SELECT Entry.blob AS entry_blob FROM Entry
        WHERE hash = :entry_hash
        ",
        named_params! {
            ":entry_hash": entry_hash,
        },
        |row| {
            Ok(from_blob::<Entry>(
                row.get(row.column_index("entry_blob")?)?,
            ))
        },
    );
    if let Err(holochain_sqlite::rusqlite::Error::QueryReturnedNoRows) = &entry {
        Ok(None)
    } else {
        Ok(Some(entry?))
    }
}
