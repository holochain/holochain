//! Functionality for safely accessing LMDB database references.

use std::path::Path;

use crate::prelude::Writer;
use crate::{db::DbKind, exports::IntegerTable, prelude::Readable};
use crate::{
    error::DatabaseResult,
    exports::{MultiTable, SingleTable},
};
use derive_more::Display;
/// Enumeration of all databases needed by Holochain
#[derive(Clone, Debug, Hash, PartialEq, Eq, Display)]
pub enum TableName {
    /// Vault database: KV store of chain entries, keyed by address
    ElementVaultPublicEntries,
    /// Vault database: KV store of chain entries, keyed by address
    ElementVaultPrivateEntries,
    /// Vault database: KV store of chain headers, keyed by address
    ElementVaultHeaders,
    /// Vault database: KVV store of chain metadata, storing relationships
    MetaVaultSys,
    /// Vault database: Kv store of links
    MetaVaultLinks,
    /// Vault database: Kv store of entry dht status
    MetaVaultMisc,
    /// int KV store storing the sequence of committed headers,
    /// most notably allowing access to the chain head
    ChainSequence,
    /// Cache database: KV store of chain entries, keyed by address
    ElementCacheEntries,
    /// Cache database: KV store of chain headers, keyed by address
    ElementCacheHeaders,
    /// Cache database: KVV store of chain metadata, storing relationships
    MetaCacheSys,
    /// Cache database: Kv store of links
    MetaCacheLinks,
    /// Vault database: Kv store of entry dht status
    MetaCacheStatus,
    /// database which stores a single key-value pair, encoding the
    /// mutable state for the entire Conductor
    ConductorState,
    /// database that stores wasm bytecode
    Wasm,
    /// database to store the [DnaDef]
    DnaDef,
    /// database to store the [EntryDef] Kvv store
    EntryDef,
    /// Authored [DhtOp]s KV store
    AuthoredDhtOps,
    /// Integrated [DhtOp]s KV store
    IntegratedDhtOps,
    /// Integration Queue of [DhtOp]s KV store where key is [DhtOpHash]
    IntegrationLimbo,
    /// Place for [DhtOp]s waiting to be validated to hang out. KV store where key is a [DhtOpHash]
    ValidationLimbo,
    /// KVV store to accumulate validation receipts for a published EntryHash
    ValidationReceipts,
    /// Single store for all known agents on the network
    Agent,
}

#[deprecated = "alias, remove"]
/// remove
pub type DbName = TableName;

/// Get access to the singleton database manager ([GetTable]),
/// in order to access individual LMDB databases
pub(super) fn initialize_databases(_path: &Path, _kind: &DbKind) -> DatabaseResult<()> {
    todo!("create database and schema if not exists");
    Ok(())
}

/// TODO
#[deprecated = "sqlite: placeholder"]
pub trait GetTable {
    /// Placeholder
    fn get_table(&self, _table_name: TableName) -> DatabaseResult<Table> {
        todo!("rewrite to return a Table")
    }

    /// Placeholder
    fn get_table_i(&self, _table_name: TableName) -> DatabaseResult<Table> {
        todo!("rewrite to return a Table")
    }

    /// Placeholder
    fn get_table_m(&self, _table_name: TableName) -> DatabaseResult<Table> {
        todo!("rewrite to return a Table")
    }
}

/// A reference to a SQLite table.
/// This patten only exists as part of the naive LMDB refactor.
#[deprecated = "lmdb: naive"]
#[derive(Clone, Debug)]
pub struct Table {}

impl Table {
    pub fn get<R: Readable, K: AsRef<[u8]>>(
        &self,
        reader: &R,
        k: K,
    ) -> StoreResult<Option<rkv::Value>> {
        todo!()
    }

    /// This handles the fact that getting from an rkv::MultiTable returns
    /// multiple results
    #[deprecated = "unneeded in the context of SQL"]
    pub fn get_m<R: Readable, K: AsRef<[u8]>>(
        &self,
        reader: &R,
        k: K,
    ) -> StoreResult<impl Iterator<Item = StoreResult<(K, Option<rkv::Value>)>>> {
        todo!();
        Ok(std::iter::empty())
    }

    pub fn put<K: AsRef<[u8]>>(
        &self,
        writer: &mut Writer,
        k: K,
        v: &rkv::Value,
    ) -> StoreResult<()> {
        todo!()
    }

    #[deprecated = "unneeded in the context of SQL"]
    pub fn put_with_flags<K: AsRef<[u8]>>(
        &self,
        writer: &mut Writer,
        k: K,
        v: &rkv::Value,
        flags: rkv::WriteFlags,
    ) -> StoreResult<()> {
        todo!()
    }

    pub fn delete<K: AsRef<[u8]>>(&self, writer: &mut Writer, k: K) -> StoreResult<()> {
        todo!()
    }

    pub fn delete_all<K: AsRef<[u8]>>(&self, writer: &mut Writer, k: K) -> StoreResult<()> {
        todo!()
    }

    /// This handles the fact that deleting from an rkv::MultiTable requires
    /// passing the value to delete (deleting a particular kv pair)
    #[deprecated = "unneeded in the context of SQL"]
    pub fn delete_m<K: AsRef<[u8]>>(
        &self,
        writer: &mut Writer,
        k: K,
        v: &rkv::Value,
    ) -> StoreResult<()> {
        todo!()
    }

    #[cfg(feature = "test_utils")]
    pub fn clear(&mut self, writer: &mut Writer) -> StoreResult<()> {
        todo!()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("Error interacting with the underlying LMDB store: {0}")]
    LmdbStoreError(#[from] failure::Compat<rkv::StoreError>),
}

pub type StoreResult<T> = Result<T, StoreError>;

impl From<rkv::StoreError> for StoreError {
    fn from(e: rkv::StoreError) -> StoreError {
        use failure::Fail;
        StoreError::LmdbStoreError(e.compat())
    }
}

impl StoreError {
    pub fn ok_if_not_found(self) -> StoreResult<()> {
        match self {
            StoreError::LmdbStoreError(err) => match err.into_inner() {
                rkv::StoreError::LmdbError(rkv::LmdbError::NotFound) => Ok(()),
                err => Err(err.into()),
            },
            err => Err(err),
        }
    }
}
