use fallible_iterator::FallibleIterator;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_serialized_bytes::prelude::*;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::Row;
use holochain_sqlite::rusqlite::Statement;
use holochain_sqlite::rusqlite::Transaction;
use holochain_sqlite::scratch::Scratch;
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
mod query;

pub mod entry;
pub mod error;
pub mod link;

pub mod prelude {
    pub use super::from_blob;
    pub use super::get_entry_from_db;
    pub use super::to_blob;
    pub use super::Params;
    pub use super::Query;
    pub use super::StateQueryResult;
    pub use super::Transactions;
    pub use holochain_sqlite::rusqlite::named_params;
    pub use holochain_sqlite::rusqlite::Row;
    pub use std::sync::Arc;
}

pub type Params<'a> = (&'a str, &'a dyn holochain_sqlite::rusqlite::ToSql);

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

pub type Transactions<'a, 'txn> = [&'a Transaction<'txn>];

pub trait Query: Clone {
    type State;
    type Data: Clone;
    type Output;

    fn create_query(&self) -> &str {
        ""
    }
    fn delete_query(&self) -> &str {
        ""
    }
    fn update_query(&self) -> &str {
        ""
    }
    fn create_params(&self) -> Vec<Params> {
        Vec::with_capacity(0)
    }
    fn delete_params(&self) -> Vec<Params> {
        Vec::with_capacity(0)
    }
    fn update_params(&self) -> Vec<Params> {
        Vec::with_capacity(0)
    }
    fn init_fold(&self) -> StateQueryResult<Self::State>;

    fn as_filter(&self) -> Box<dyn Fn(&Self::Data) -> bool> {
        Box::new(|_| true)
    }

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Data>>;

    fn fold(&mut self, state: Self::State, data: Self::Data) -> StateQueryResult<Self::State>;

    fn render(
        &mut self,
        state: Self::State,
        txns: &Transactions<'_, '_>,
    ) -> StateQueryResult<Self::Output>;

    fn run(
        &mut self,
        txns: &Transactions<'_, '_>,
        scratch: Option<&Scratch<Self::Data>>,
    ) -> StateQueryResult<Self::Output> {
        let mut stmts: Vec<_> = fallible_iterator::convert(
            txns.into_iter()
                .map(|txn| QueryStmt::new(txn, self.clone())),
        )
        .collect()?;
        let map_fn = self.as_map();
        let iter = stmts.iter_mut().map(|stmt| Ok(stmt.iter(map_fn.clone())?));
        let iter = fallible_iterator::convert(iter).flatten();
        let scratch = scratch.map(|s| s.filter(self.as_filter()).map_err(StateQueryError::from));
        let result = match scratch {
            Some(scratch) => {
                let iter = iter.chain(scratch);
                iter.fold(self.init_fold()?, |state, i| self.fold(state, i))?
            }
            None => iter.fold(self.init_fold()?, |state, i| self.fold(state, i))?,
        };
        drop(stmts);
        self.render(result, txns)
    }
}

struct QueryStmt<'stmt, Q: Query> {
    create_stmt: Option<Statement<'stmt>>,
    delete_stmt: Option<Statement<'stmt>>,
    update_stmt: Option<Statement<'stmt>>,
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
        let create_stmt = new_stmt(query.create_query())?;
        let delete_stmt = new_stmt(query.delete_query())?;
        let update_stmt = new_stmt(query.update_query())?;

        Ok(Self {
            create_stmt,
            delete_stmt,
            update_stmt,
            query,
        })
    }
    fn iter<T: 'iter>(
        &'iter mut self,
        map_fn: std::sync::Arc<dyn Fn(&Row) -> StateQueryResult<T>>,
    ) -> StateQueryResult<StmtIter<'iter, T>> {
        let creates = Self::new_iter(
            &self.query.create_params(),
            self.create_stmt.as_mut(),
            map_fn.clone(),
        )?;
        let deletes = Self::new_iter(
            &self.query.delete_params(),
            self.delete_stmt.as_mut(),
            map_fn.clone(),
        )?;
        let updates = Self::new_iter(
            &self.query.update_params(),
            self.update_stmt.as_mut(),
            map_fn.clone(),
        )?;
        Ok(Box::new(creates.chain(deletes).chain(updates)))
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

pub(crate) fn row_to_header(row: &Row) -> StateQueryResult<SignedHeaderHashed> {
    let header = from_blob::<SignedHeader>(row.get(row.column_index("header_blob")?)?);
    let SignedHeader(header, signature) = header;
    let header = HeaderHashed::from_content_sync(header);
    let shh = SignedHeaderHashed::with_presigned(header, signature);
    Ok(shh)
}

pub fn to_blob<T: Serialize + std::fmt::Debug>(t: T) -> Vec<u8> {
    holochain_serialized_bytes::encode(&t).unwrap()
}

pub fn from_blob<T: DeserializeOwned + std::fmt::Debug>(blob: Vec<u8>) -> T {
    holochain_serialized_bytes::decode(&blob).unwrap()
}

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
