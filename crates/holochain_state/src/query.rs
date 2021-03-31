use fallible_iterator::FallibleIterator;
use holo_hash::HeaderHash;
use holochain_serialized_bytes::prelude::*;
use holochain_sqlite::rusqlite::Row;
use holochain_sqlite::rusqlite::Statement;
use holochain_sqlite::rusqlite::Transaction;
use holochain_sqlite::scratch::Scratch;
use holochain_zome_types::Header;
use holochain_zome_types::HeaderHashed;
use holochain_zome_types::SignedHeader;
use holochain_zome_types::SignedHeaderHashed;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::collections::HashSet;
use thiserror::Error;

#[cfg(test)]
mod query;

pub mod entry;
pub mod link;

#[derive(Error, Debug)]
pub struct PlaceHolderError;

impl std::fmt::Display for PlaceHolderError {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl From<holochain_sqlite::rusqlite::Error> for PlaceHolderError {
    fn from(e: holochain_sqlite::rusqlite::Error) -> Self {
        tracing::error!(?e);
        todo!()
    }
}
impl From<std::convert::Infallible> for PlaceHolderError {
    fn from(_: std::convert::Infallible) -> Self {
        unreachable!()
    }
}
impl From<holochain_sqlite::error::DatabaseError> for PlaceHolderError {
    fn from(_: holochain_sqlite::error::DatabaseError) -> Self {
        todo!()
    }
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
    fn init_fold(&self) -> Result<Self::State, PlaceHolderError>;

    fn as_filter(&self) -> Box<dyn Fn(&Header) -> bool>;

    fn fold(
        &mut self,
        state: Self::State,
        header: SignedHeaderHashed,
    ) -> Result<Self::State, PlaceHolderError>;
    fn render(
        &mut self,
        state: Self::State,
        txns: &Transactions<'_, '_>,
    ) -> Result<Self::Output, PlaceHolderError>;

    fn run(
        &mut self,
        txns: &Transactions<'_, '_>,
        scratch: Option<&Scratch>,
    ) -> Result<Self::Output, PlaceHolderError> {
        let mut stmts: Vec<_> = txns
            .into_iter()
            .map(|txn| QueryStmt::new(txn, self.clone()))
            .collect();
        let iter = stmts.iter_mut().map(|stmt| Ok(stmt.iter()));
        let iter = fallible_iterator::convert(iter).flatten();
        let scratch = scratch.map(|s| s.filter(self.as_filter()).map_err(PlaceHolderError::from));
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
    create_stmt: Statement<'stmt>,
    delete_stmt: Statement<'stmt>,
    query: Q,
}

impl<'stmt, 'iter, Q: Query> QueryStmt<'stmt, Q> {
    fn new(txn: &'stmt Transaction, query: Q) -> Self {
        let create_stmt = txn.prepare(&query.create_query()).unwrap();
        let delete_stmt = txn.prepare(&query.delete_query()).unwrap();
        Self {
            create_stmt,
            delete_stmt,
            query,
        }
    }
    fn iter(
        &'iter mut self,
    ) -> impl FallibleIterator<Item = SignedHeaderHashed, Error = PlaceHolderError> + 'iter {
        let creates = self
            .create_stmt
            .query_and_then_named(&self.query.create_params(), row_to_header)
            .unwrap();

        let deletes = self
            .delete_stmt
            .query_and_then_named(&self.query.delete_params(), row_to_header)
            .unwrap();
        let creates = fallible_iterator::convert(creates);
        let deletes = fallible_iterator::convert(deletes);
        creates.chain(deletes)
    }
}

fn row_to_header(row: &Row) -> Result<SignedHeaderHashed, PlaceHolderError> {
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
