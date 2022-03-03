use crate::scratch::FilteredScratch;
use crate::scratch::Scratch;
use fallible_iterator::FallibleIterator;
use holo_hash::hash_type::AnyDht;
use holo_hash::AgentPubKey;
use holo_hash::AnyDhtHash;
use holo_hash::DhtOpHash;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_serialized_bytes::prelude::*;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::Row;
use holochain_sqlite::rusqlite::Statement;
use holochain_sqlite::rusqlite::Transaction;
use holochain_sqlite::sql::sql_cell::FETCH_OP;
use holochain_types::dht_op::DhtOp;
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::dht_op::DhtOpType;
use holochain_types::prelude::HasValidationStatus;
use holochain_types::prelude::Judged;
use holochain_zome_types::Element;
use holochain_zome_types::Entry;
use holochain_zome_types::EntryVisibility;
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
pub mod element_details;
pub mod entry_details;
pub mod error;
pub mod link;
pub mod link_details;
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

/// Helper for getting to the inner Data type of the Item of a Query
pub type QueryData<Q> = <<Q as Query>::Item as HasValidationStatus>::Data;

/// You should keep your query type cheap to clone.
/// If there is any large data put it in an Arc.
pub trait Query: Clone {
    type State;
    type Item: HasValidationStatus;
    type Output;

    fn query(&self) -> String {
        "".into()
    }
    fn params(&self) -> Vec<Params> {
        Vec::with_capacity(0)
    }
    fn init_fold(&self) -> StateQueryResult<Self::State>;

    fn as_filter(&self) -> Box<dyn Fn(&QueryData<Self>) -> bool> {
        Box::new(|_| true)
    }

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Item>>;

    fn fold(&self, state: Self::State, data: Self::Item) -> StateQueryResult<Self::State>;

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
    type O: StoresIter<Q::Item>;

    /// Gets the raw initial data from the database, needed to begin the query.
    // MD: can the query be &Q?
    fn get_initial_data(&self, query: Q) -> StateQueryResult<Self::O>;
}

/// Queries that can have access to private data will
/// implement this trait.
pub trait PrivateDataQuery {
    type Hash;

    /// Construct the query with access to private data for this agent.
    fn with_private_data_access(hash: Self::Hash, author: Arc<AgentPubKey>) -> Self;

    /// Construct the query without access to private data.
    fn without_private_data_access(hash: Self::Hash) -> Self;
}

pub trait Store {
    /// Get an [`Entry`] from this store.
    fn get_entry(&self, hash: &EntryHash) -> StateQueryResult<Option<Entry>>;

    /// Get an [`Entry`] from this store.
    /// - Will return any public entry.
    /// - If an author is provided
    /// and a header for this entry matches
    /// the author then any entry will be return
    /// regardless of visibility .
    fn get_public_or_authored_entry(
        &self,
        hash: &EntryHash,
        author: Option<&AgentPubKey>,
    ) -> StateQueryResult<Option<Entry>>;

    /// Get an [`SignedHeaderHashed`] from this store.
    fn get_header(&self, hash: &HeaderHash) -> StateQueryResult<Option<SignedHeaderHashed>>;

    /// Get an [`Element`] from this store.
    fn get_element(&self, hash: &AnyDhtHash) -> StateQueryResult<Option<Element>>;

    /// Get an [`Element`] from this store that is either public or
    /// authored by the given key.
    fn get_public_or_authored_element(
        &self,
        hash: &AnyDhtHash,
        author: Option<&AgentPubKey>,
    ) -> StateQueryResult<Option<Element>>;

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
    fn iter(&mut self) -> StateQueryResult<StmtIter<'_, T>>;
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
    Q: Query<Item = Judged<SignedHeaderHashed>>,
{
    stmts: QueryStmts<'stmt, Q>,
    filtered_scratch: FilteredScratch,
}

impl<'stmt, Q: Query> Stores<Q> for Txn<'stmt, '_> {
    type O = QueryStmt<'stmt, Q>;

    fn get_initial_data(&self, query: Q) -> StateQueryResult<Self::O> {
        QueryStmt::new(self.txn, query)
    }
}

impl<'stmt> Store for Txn<'stmt, '_> {
    fn get_entry(&self, hash: &EntryHash) -> StateQueryResult<Option<Entry>> {
        get_entry_from_db(self.txn, hash)
    }

    fn get_public_or_authored_entry(
        &self,
        hash: &EntryHash,
        author: Option<&AgentPubKey>,
    ) -> StateQueryResult<Option<Entry>> {
        // Try to get the entry if it's public.
        match get_public_entry_from_db(self.txn, hash)? {
            Some(e) => Ok(Some(e)),
            None => match author {
                // If no public entry is found try to find
                // any authored by this agent.
                Some(author) => Ok(self
                    .get_any_authored_element(hash, author)?
                    .and_then(|el| el.into_inner().1.into_option())),
                None => Ok(None),
            },
        }
    }

    fn contains_entry(&self, hash: &EntryHash) -> StateQueryResult<bool> {
        let exists = self.txn.query_row_named(
            "
            SELECT
            EXISTS(
                SELECT 1 FROM Entry
                WHERE hash = :hash
            )
            ",
            named_params! {
                ":hash": hash,
            },
            |row| {
                let exists: i32 = row.get(0)?;
                if exists == 1 {
                    Ok(true)
                } else {
                    Ok(false)
                }
            },
        );
        if let Err(holochain_sqlite::rusqlite::Error::QueryReturnedNoRows) = &exists {
            Ok(false)
        } else {
            Ok(exists?)
        }
    }

    fn contains_header(&self, hash: &HeaderHash) -> StateQueryResult<bool> {
        let exists = self.txn.query_row_named(
            "
            SELECT
            EXISTS(
                SELECT 1 FROM Header
                WHERE hash = :hash
            )
            ",
            named_params! {
                ":hash": hash,
            },
            |row| {
                let exists: i32 = row.get(0)?;
                if exists == 1 {
                    Ok(true)
                } else {
                    Ok(false)
                }
            },
        );
        if let Err(holochain_sqlite::rusqlite::Error::QueryReturnedNoRows) = &exists {
            Ok(false)
        } else {
            Ok(exists?)
        }
    }

    fn get_header(&self, hash: &HeaderHash) -> StateQueryResult<Option<SignedHeaderHashed>> {
        let shh = self.txn.query_row_named(
            "
            SELECT
            Header.blob, Header.hash
            FROM Header
            WHERE hash = :hash
            ",
            named_params! {
                ":hash": hash,
            },
            |row| {
                let header =
                    from_blob::<SignedHeader>(row.get(row.as_ref().column_index("blob")?)?);
                Ok(header.and_then(|header| {
                    let SignedHeader(header, signature) = header;
                    let hash: HeaderHash = row.get(row.as_ref().column_index("hash")?)?;
                    let header = HeaderHashed::with_pre_hashed(header, hash);
                    let shh = SignedHeaderHashed::with_presigned(header, signature);
                    Ok(shh)
                }))
            },
        );
        if let Err(holochain_sqlite::rusqlite::Error::QueryReturnedNoRows) = &shh {
            Ok(None)
        } else {
            Ok(Some(shh??))
        }
    }

    fn get_element(&self, hash: &AnyDhtHash) -> StateQueryResult<Option<Element>> {
        match *hash.hash_type() {
            AnyDht::Entry => self.get_any_element(&hash.clone().into()),
            AnyDht::Header => self.get_exact_element(&hash.clone().into()),
        }
    }

    fn get_public_or_authored_element(
        &self,
        hash: &AnyDhtHash,
        author: Option<&AgentPubKey>,
    ) -> StateQueryResult<Option<Element>> {
        match *hash.hash_type() {
            // Try to get a public element.
            AnyDht::Entry => match self.get_any_public_element(&hash.clone().into())? {
                Some(el) => Ok(Some(el)),
                None => match author {
                    // If there are none try to get a private authored element.
                    Some(author) => self.get_any_authored_element(&hash.clone().into(), author),
                    // If there are no private authored elements then try to get any element and
                    // remove the entry.
                    None => Ok(self
                        .get_any_element(&hash.clone().into())?
                        .map(|el| Element::new(el.into_inner().0, None))),
                },
            },
            AnyDht::Header => Ok(self.get_exact_element(&hash.clone().into())?.map(|el| {
                // Filter out the entry if it's private.
                let is_private_entry = el.header().entry_type().map_or(false, |et| {
                    matches!(et.visibility(), EntryVisibility::Private)
                });
                if is_private_entry {
                    Element::new(el.into_inner().0, None)
                } else {
                    el
                }
            })),
        }
    }
}

impl<'stmt> Txn<'stmt, '_> {
    fn get_exact_element(&self, hash: &HeaderHash) -> StateQueryResult<Option<Element>> {
        let element = self.txn.query_row_named(
            "
            SELECT
            Header.blob AS header_blob, Header.hash, Entry.blob as entry_blob
            FROM Header
            LEFT JOIN Entry ON Header.entry_hash = Entry.hash
            WHERE
            Header.hash = :hash
            ",
            named_params! {
                ":hash": hash,
            },
            |row| {
                let header =
                    from_blob::<SignedHeader>(row.get(row.as_ref().column_index("header_blob")?)?);
                Ok(header.and_then(|header| {
                    let SignedHeader(header, signature) = header;
                    let hash: HeaderHash = row.get(row.as_ref().column_index("hash")?)?;
                    let header = HeaderHashed::with_pre_hashed(header, hash);
                    let shh = SignedHeaderHashed::with_presigned(header, signature);
                    let entry: Option<Vec<u8>> =
                        row.get(row.as_ref().column_index("entry_blob")?)?;
                    let entry = match entry {
                        Some(entry) => Some(from_blob::<Entry>(entry)?),
                        None => None,
                    };
                    Ok(Element::new(shh, entry))
                }))
            },
        );
        if let Err(holochain_sqlite::rusqlite::Error::QueryReturnedNoRows) = &element {
            Ok(None)
        } else {
            Ok(Some(element??))
        }
    }
    fn get_any_element(&self, hash: &EntryHash) -> StateQueryResult<Option<Element>> {
        let element = self.txn.query_row_named(
            "
            SELECT
            Header.blob AS header_blob, Header.hash, Entry.blob as entry_blob
            FROM Header
            JOIN Entry ON Header.entry_hash = Entry.hash
            WHERE
            Entry.hash = :hash
            ",
            named_params! {
                ":hash": hash,
            },
            |row| {
                let header =
                    from_blob::<SignedHeader>(row.get(row.as_ref().column_index("header_blob")?)?);
                Ok(header.and_then(|header| {
                    let SignedHeader(header, signature) = header;
                    let hash: HeaderHash = row.get(row.as_ref().column_index("hash")?)?;
                    let header = HeaderHashed::with_pre_hashed(header, hash);
                    let shh = SignedHeaderHashed::with_presigned(header, signature);
                    let entry: Option<Vec<u8>> =
                        row.get(row.as_ref().column_index("entry_blob")?)?;
                    let entry = match entry {
                        Some(entry) => Some(from_blob::<Entry>(entry)?),
                        None => None,
                    };
                    Ok(Element::new(shh, entry))
                }))
            },
        );
        if let Err(holochain_sqlite::rusqlite::Error::QueryReturnedNoRows) = &element {
            Ok(None)
        } else {
            Ok(Some(element??))
        }
    }

    fn get_any_public_element(&self, hash: &EntryHash) -> StateQueryResult<Option<Element>> {
        let element = self.txn.query_row_named(
            "
            SELECT
            Header.blob AS header_blob, Header.hash, Entry.blob as entry_blob
            FROM Header
            JOIN Entry ON Header.entry_hash = Entry.hash
            WHERE
            Entry.hash = :hash
            AND
            Header.private_entry = 0
            ",
            named_params! {
                ":hash": hash,
            },
            |row| {
                let header =
                    from_blob::<SignedHeader>(row.get(row.as_ref().column_index("header_blob")?)?);
                Ok(header.and_then(|header| {
                    let SignedHeader(header, signature) = header;
                    let hash: HeaderHash = row.get(row.as_ref().column_index("hash")?)?;
                    let header = HeaderHashed::with_pre_hashed(header, hash);
                    let shh = SignedHeaderHashed::with_presigned(header, signature);
                    let entry: Option<Vec<u8>> =
                        row.get(row.as_ref().column_index("entry_blob")?)?;
                    let entry = match entry {
                        Some(entry) => Some(from_blob::<Entry>(entry)?),
                        None => None,
                    };
                    Ok(Element::new(shh, entry))
                }))
            },
        );
        if let Err(holochain_sqlite::rusqlite::Error::QueryReturnedNoRows) = &element {
            Ok(None)
        } else {
            Ok(Some(element??))
        }
    }

    fn get_any_authored_element(
        &self,
        hash: &EntryHash,
        author: &AgentPubKey,
    ) -> StateQueryResult<Option<Element>> {
        let element = self.txn.query_row_named(
            "
            SELECT
            Header.blob AS header_blob, Header.hash, Entry.blob as entry_blob
            FROM Header
            JOIN Entry ON Header.entry_hash = Entry.hash
            WHERE
            Entry.hash = :hash
            AND
            Header.author = :author
            ",
            named_params! {
                ":hash": hash,
                ":author": author,
            },
            |row| {
                let header =
                    from_blob::<SignedHeader>(row.get(row.as_ref().column_index("header_blob")?)?);
                Ok(header.and_then(|header| {
                    let SignedHeader(header, signature) = header;
                    let hash: HeaderHash = row.get(row.as_ref().column_index("hash")?)?;
                    let header = HeaderHashed::with_pre_hashed(header, hash);
                    let shh = SignedHeaderHashed::with_presigned(header, signature);
                    let entry: Option<Vec<u8>> =
                        row.get(row.as_ref().column_index("entry_blob")?)?;
                    let entry = match entry {
                        Some(entry) => Some(from_blob::<Entry>(entry)?),
                        None => None,
                    };
                    Ok(Element::new(shh, entry))
                }))
            },
        );
        if let Err(holochain_sqlite::rusqlite::Error::QueryReturnedNoRows) = &element {
            Ok(None)
        } else {
            Ok(Some(element??))
        }
    }
}

impl<'stmt, Q: Query> StoresIter<Q::Item> for QueryStmt<'stmt, Q> {
    fn iter(&mut self) -> StateQueryResult<StmtIter<'_, Q::Item>> {
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

    fn get_header(&self, hash: &HeaderHash) -> StateQueryResult<Option<SignedHeaderHashed>> {
        for txn in &self.txns {
            let r = txn.get_header(hash)?;
            if r.is_some() {
                return Ok(r);
            }
        }
        Ok(None)
    }

    fn get_element(&self, hash: &AnyDhtHash) -> StateQueryResult<Option<Element>> {
        for txn in &self.txns {
            let r = txn.get_element(hash)?;
            if r.is_some() {
                return Ok(r);
            }
        }
        Ok(None)
    }

    fn get_public_or_authored_entry(
        &self,
        hash: &EntryHash,
        author: Option<&AgentPubKey>,
    ) -> StateQueryResult<Option<Entry>> {
        for txn in &self.txns {
            let r = txn.get_public_or_authored_entry(hash, author)?;
            if r.is_some() {
                return Ok(r);
            }
        }
        Ok(None)
    }

    fn get_public_or_authored_element(
        &self,
        hash: &AnyDhtHash,
        author: Option<&AgentPubKey>,
    ) -> StateQueryResult<Option<Element>> {
        for txn in &self.txns {
            let r = txn.get_public_or_authored_element(hash, author)?;
            if r.is_some() {
                return Ok(r);
            }
        }
        Ok(None)
    }
}

impl<'stmt, Q: Query> StoresIter<Q::Item> for QueryStmts<'stmt, Q> {
    fn iter(&mut self) -> StateQueryResult<StmtIter<'_, Q::Item>> {
        Ok(Box::new(
            fallible_iterator::convert(self.stmts.iter_mut().map(Ok)).flat_map(|stmt| stmt.iter()),
        ))
    }
}

impl<'borrow, 'txn, Q> Stores<Q> for DbScratch<'borrow, 'txn>
where
    Q: Query<Item = Judged<SignedHeaderHashed>>,
{
    type O = DbScratchIter<'borrow, Q>;

    fn get_initial_data(&self, query: Q) -> StateQueryResult<Self::O> {
        Ok(DbScratchIter {
            stmts: self.txns.get_initial_data(query.clone())?,
            filtered_scratch: self.scratch.get_initial_data(query)?,
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

    fn get_header(&self, hash: &HeaderHash) -> StateQueryResult<Option<SignedHeaderHashed>> {
        let r = self.txns.get_header(hash)?;
        if r.is_none() {
            self.scratch.get_header(hash)
        } else {
            Ok(r)
        }
    }

    fn get_element(&self, hash: &AnyDhtHash) -> StateQueryResult<Option<Element>> {
        let r = self.txns.get_element(hash)?;
        if r.is_none() {
            self.scratch.get_element(hash)
        } else {
            Ok(r)
        }
    }

    fn get_public_or_authored_entry(
        &self,
        hash: &EntryHash,
        author: Option<&AgentPubKey>,
    ) -> StateQueryResult<Option<Entry>> {
        let r = self.txns.get_public_or_authored_entry(hash, author)?;
        if r.is_none() {
            // Entries in the scratch are authored by definition.
            self.scratch.get_entry(hash)
        } else {
            Ok(r)
        }
    }

    fn get_public_or_authored_element(
        &self,
        hash: &AnyDhtHash,
        author: Option<&AgentPubKey>,
    ) -> StateQueryResult<Option<Element>> {
        let r = self.txns.get_public_or_authored_element(hash, author)?;
        if r.is_none() {
            // Elements in the scratch are authored by definition.
            self.scratch.get_element(hash)
        } else {
            Ok(r)
        }
    }
}

impl<'stmt, Q> StoresIter<Q::Item> for DbScratchIter<'stmt, Q>
where
    Q: Query<Item = Judged<SignedHeaderHashed>>,
{
    fn iter(&mut self) -> StateQueryResult<StmtIter<'_, Q::Item>> {
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

impl<'borrow, 'txn> From<&'borrow mut Transaction<'txn>> for Txn<'borrow, 'txn> {
    fn from(txn: &'borrow mut Transaction<'txn>) -> Self {
        Self { txn }
    }
}

impl<'borrow, 'txn> From<&'borrow Transactions<'borrow, 'txn>> for Txns<'borrow, 'txn> {
    fn from(txns: &'borrow Transactions<'borrow, 'txn>) -> Self {
        let txns = txns.iter().map(|&txn| Txn::from(txn)).collect();
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

pub(crate) type StmtIter<'iter, T> =
    Box<dyn FallibleIterator<Item = T, Error = StateQueryError> + 'iter>;

impl<'stmt, 'iter, Q: Query> QueryStmt<'stmt, Q> {
    fn new(txn: &'stmt Transaction, query: Q) -> StateQueryResult<Self> {
        let new_stmt = |q: &str| {
            if q.is_empty() {
                Ok(None)
            } else {
                StateQueryResult::Ok(Some(txn.prepare(q)?))
            }
        };
        let stmt = new_stmt(&query.query())?;

        Ok(Self { stmt, query })
    }
    fn iter(&'iter mut self) -> StateQueryResult<StmtIter<'iter, Q::Item>> {
        let map_fn = self.query.as_map();
        let iter = Self::new_iter(&self.query.params(), self.stmt.as_mut(), map_fn.clone())?;
        Ok(Box::new(iter))
    }

    fn new_iter<T: 'iter>(
        params: &[Params],
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
        let header = from_blob::<SignedHeader>(row.get(blob_index)?)?;
        let SignedHeader(header, signature) = header;
        let hash: HeaderHash = row.get(row.as_ref().column_index(hash_index)?)?;
        let header = HeaderHashed::with_pre_hashed(header, hash);
        let shh = SignedHeaderHashed::with_presigned(header, signature);
        Ok(shh)
    }
}

pub fn row_blob_to_header(
    blob_index: &'static str,
) -> impl Fn(&Row) -> StateQueryResult<SignedHeaderHashed> {
    move |row| {
        let header = from_blob::<SignedHeader>(row.get(blob_index)?)?;
        let SignedHeader(header, signature) = header;
        let header = HeaderHashed::from_content_sync(header);
        let shh = SignedHeaderHashed::with_presigned(header, signature);
        Ok(shh)
    }
}

/// Serialize a value to be stored in a database as a BLOB type
pub fn to_blob<T: Serialize + std::fmt::Debug>(t: &T) -> StateQueryResult<Vec<u8>> {
    Ok(holochain_serialized_bytes::encode(t)?)
}

/// Deserialize a BLOB from a database into a value
pub fn from_blob<T: DeserializeOwned + std::fmt::Debug>(blob: Vec<u8>) -> StateQueryResult<T> {
    Ok(holochain_serialized_bytes::decode(&blob)?)
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
                row.get(row.as_ref().column_index("entry_blob")?)?,
            ))
        },
    );
    if let Err(holochain_sqlite::rusqlite::Error::QueryReturnedNoRows) = &entry {
        Ok(None)
    } else {
        Ok(Some(entry??))
    }
}

/// Fetch a public Entry from a DB by its hash.
pub fn get_public_entry_from_db(
    txn: &Transaction,
    entry_hash: &EntryHash,
) -> StateQueryResult<Option<Entry>> {
    let entry = txn.query_row_named(
        "
        SELECT Entry.blob AS entry_blob FROM Entry
        JOIN Header ON Header.entry_hash = Entry.hash
        WHERE Entry.hash = :entry_hash
        AND
        Header.private_entry = 0
        ",
        named_params! {
            ":entry_hash": entry_hash,
        },
        |row| {
            Ok(from_blob::<Entry>(
                row.get(row.as_ref().column_index("entry_blob")?)?,
            ))
        },
    );
    if let Err(holochain_sqlite::rusqlite::Error::QueryReturnedNoRows) = &entry {
        Ok(None)
    } else {
        Ok(Some(entry??))
    }
}

/// Get a [`DhtOp`] from the database
/// filtering out private entries and
/// [`DhtOp::StoreEntry`] where the entry
/// is private.
/// The ops are suitable for publishing / gossiping.
pub fn get_public_op_from_db(
    txn: &Transaction,
    op_hash: &DhtOpHash,
) -> StateQueryResult<Option<DhtOpHashed>> {
    let result = txn.query_row_and_then(
        FETCH_OP,
        named_params! {
            ":hash": op_hash,
        },
        |row| {
            let hash: DhtOpHash = row.get("hash")?;
            let op_hashed =
                map_sql_dht_op_common(row)?.map(|op| DhtOpHashed::with_pre_hashed(op, hash));
            StateQueryResult::Ok(op_hashed)
        },
    );
    match result {
        Err(StateQueryError::Sql(holochain_sqlite::rusqlite::Error::QueryReturnedNoRows)) => {
            Ok(None)
        }
        Err(e) => Err(e),
        Ok(result) => Ok(result),
    }
}

pub fn map_sql_dht_op_common(row: &Row) -> StateQueryResult<Option<DhtOp>> {
    let header = from_blob::<SignedHeader>(row.get("header_blob")?)?;
    let op_type: DhtOpType = row.get("type")?;
    if header
        .0
        .entry_type()
        .map_or(false, |et| *et.visibility() == EntryVisibility::Private)
        && op_type == DhtOpType::StoreEntry
    {
        return Ok(None);
    }

    // Check that the entry isn't private before gossiping it.
    let mut entry: Option<Entry> = None;
    if header
        .0
        .entry_type()
        .filter(|et| *et.visibility() == EntryVisibility::Public)
        .is_some()
    {
        let e: Option<Vec<u8>> = row.get("entry_blob")?;
        entry = match e {
            Some(entry) => Some(from_blob::<Entry>(entry)?),
            None => None,
        };
    }
    Ok(Some(DhtOp::from_type(op_type, header, entry)?))
}
