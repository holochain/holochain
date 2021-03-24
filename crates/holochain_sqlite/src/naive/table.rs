//! Functionality for safely accessing databases.

use crate::prelude::*;
use crate::{buffer::iter::SqlIter, error::DatabaseResult};
use crate::{db::DbKind, prelude::Readable};
use derive_more::Display;
use rusqlite::{types::Value, *};

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

    #[cfg(feature = "test_utils")]
    TestSingle(String),

    #[cfg(feature = "test_utils")]
    TestMulti(String),
}

impl TableName {
    /// Associates a [TableKind] to each [TableName]
    pub fn kind(&self) -> TableKind {
        use TableKind::*;
        use TableName::*;
        match self {
            ElementVaultPublicEntries => Single,
            ElementVaultPrivateEntries => Single,
            ElementVaultHeaders => Single,
            MetaVaultSys => Multi,
            MetaVaultLinks => Single,
            MetaVaultMisc => Single,
            ChainSequence => Single, // int
            ElementCacheEntries => Single,
            ElementCacheHeaders => Single,
            MetaCacheSys => Multi,
            MetaCacheLinks => Single,
            MetaCacheStatus => Single,
            ConductorState => Single,
            Wasm => Single,
            DnaDef => Single,
            EntryDef => Single,
            AuthoredDhtOps => Single,
            IntegratedDhtOps => Single,
            IntegrationLimbo => Single,
            ValidationLimbo => Single,
            ValidationReceipts => Multi,
            Agent => Single,
            #[cfg(feature = "test_utils")]
            TestSingle(_) => Single,
            #[cfg(feature = "test_utils")]
            TestMulti(_) => Multi,
        }
    }
}

impl ToSql for TableName {
    fn to_sql(&self) -> Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Owned(
            format!("{}", self).into(),
        ))
    }
}

pub enum TableKind {
    Single,
    Multi,
}

impl TableKind {
    pub fn is_single(&self) -> bool {
        matches!(self, Self::Single)
    }

    pub fn is_multi(&self) -> bool {
        matches!(self, Self::Multi)
    }
}

pub(crate) fn initialize_table_single(
    conn: &mut Connection,
    table_name: String,
) -> rusqlite::Result<()> {
    // create table
    conn.execute(
        &format!(
            "CREATE TABLE IF NOT EXISTS {} (
                key       BLOB PRIMARY KEY,
                val       BLOB NOT NULL
            );",
            table_name
        ),
        NO_PARAMS,
    )?;

    Ok(())
}

pub(crate) fn initialize_table_multi(
    conn: &mut Connection,
    table_name: String,
) -> rusqlite::Result<()> {
    // create table
    conn.execute(
        &format!(
            "CREATE TABLE IF NOT EXISTS {0} (
                key       BLOB NOT NULL,
                val       BLOB NOT NULL,
                CONSTRAINT {0}_pk PRIMARY KEY (key, val)
            );",
            table_name,
        ),
        NO_PARAMS,
    )?;

    // create two indexes, one unique
    let key_index_name = format!("{}_idx_k", table_name);
    conn.execute(
        &format!(
            "CREATE INDEX IF NOT EXISTS {} ON {} ( key );",
            key_index_name, table_name
        ),
        NO_PARAMS,
    )?;
    Ok(())
}

fn initialize_table(conn: &mut Connection, name: TableName) -> rusqlite::Result<()> {
    let table_name = format!("{}", name);

    match name.kind() {
        TableKind::Single => initialize_table_single(conn, table_name),
        TableKind::Multi => initialize_table_multi(conn, table_name),
    }
}

pub(crate) fn initialize_database(conn: &mut Connection, db_kind: &DbKind) -> rusqlite::Result<()> {
    match db_kind {
        DbKind::Cell(_) => {
            initialize_table(conn, TableName::ElementVaultPublicEntries)?;
            initialize_table(conn, TableName::ElementVaultPrivateEntries)?;
            initialize_table(conn, TableName::ElementVaultHeaders)?;
            initialize_table(conn, TableName::MetaVaultSys)?;
            initialize_table(conn, TableName::MetaVaultLinks)?;
            initialize_table(conn, TableName::MetaVaultMisc)?;
            initialize_table(conn, TableName::ChainSequence)?;
            initialize_table(conn, TableName::ElementCacheEntries)?;
            initialize_table(conn, TableName::ElementCacheHeaders)?;
            initialize_table(conn, TableName::MetaCacheSys)?;
            initialize_table(conn, TableName::MetaCacheLinks)?;
            initialize_table(conn, TableName::MetaCacheStatus)?;
            initialize_table(conn, TableName::AuthoredDhtOps)?;
            initialize_table(conn, TableName::IntegratedDhtOps)?;
            initialize_table(conn, TableName::IntegrationLimbo)?;
            initialize_table(conn, TableName::ValidationLimbo)?;
            initialize_table(conn, TableName::ValidationReceipts)?;

            crate::schema::SCHEMA_CELL.initialize(conn, db_kind)?;
        }
        DbKind::Conductor => {
            initialize_table(conn, TableName::ConductorState)?;

            // crate::schema::SCHEMA_CONDUCTOR.initialize(conn, db_kind)?;
        }
        DbKind::Wasm => {
            initialize_table(conn, TableName::Wasm)?;
            initialize_table(conn, TableName::DnaDef)?;
            initialize_table(conn, TableName::EntryDef)?;

            // crate::schema::SCHEMA_WASM.initialize(conn, db_kind)?;
        }
        DbKind::P2p => {
            initialize_table(conn, TableName::Agent)?;

            // crate::schema::SCHEMA_P2P.initialize(conn, db_kind)?;
        }
    }
    Ok(())
}

/// This trait used to make sense when we were using LMDB.
/// Now it's kind of silly. TODO: remove.
pub trait GetTable {
    /// Get a reference to a table by name.
    // This doesn't need to be a method at all.
    fn get_table(&self, name: TableName) -> DatabaseResult<Table> {
        Ok(Table { name })
    }
}

/// A reference to a SQLite table.
/// This patten only exists as part of the naive LMDB refactor.
/// It directly replaces LMDB's Database.
#[derive(Clone, Debug)]
pub struct Table {
    pub(crate) name: TableName,
}

impl Table {
    pub fn name(&self) -> &TableName {
        &self.name
    }

    pub fn kind(&self) -> TableKind {
        self.name.kind()
    }

    /// TODO: would be amazing if this could return a ValueRef instead.
    ///       but I don't think it's possible. Could use a macro instead...
    pub fn get<R: Readable, K: ToSql>(
        &self,
        reader: &mut R,
        k: K,
    ) -> DatabaseResult<Option<Value>> {
        Ok(reader.get(self, k)?)
    }

    /// Get all key-value pairs for a given key on a TableKind::Multi table.
    /// Calling this on a Single table is a mistake, and there is no type-level
    /// enforcement of this.
    pub fn get_multi<R: Readable, K: ToSql>(
        &self,
        reader: &mut R,
        k: &K,
    ) -> DatabaseResult<SqlIter> {
        Ok(reader.get_multi(self, k)?)
    }

    pub fn put<K: ToSql>(&self, txn: &mut Writer, k: &K, v: &Value) -> DatabaseResult<()> {
        crate::transaction::put_kv(txn, self, k, v)
    }

    pub fn delete<K: ToSql>(&self, txn: &mut Writer, k: &K) -> DatabaseResult<()> {
        delete_single(txn, self, k)
    }

    pub fn delete_all<K: ToSql>(&self, txn: &mut Writer, k: &K) -> DatabaseResult<()> {
        delete_multi(txn, self, k)
    }

    pub fn delete_kv<K: ToSql>(&self, txn: &mut Writer, k: &K, v: &Value) -> DatabaseResult<()> {
        delete_kv(txn, self, k, v)
    }

    pub fn iter_start<R: Readable>(&self, reader: &mut R) -> DatabaseResult<SqlIter> {
        reader.iter_start(self)
    }

    pub fn iter_end<R: Readable>(&self, reader: &mut R) -> DatabaseResult<SqlIter> {
        reader.iter_end(self)
    }

    pub fn iter_from<R: Readable, K: ToSql>(
        &self,
        reader: &mut R,
        k: &K,
    ) -> DatabaseResult<SqlIter> {
        reader.iter_from(self, k)
    }

    #[cfg(feature = "test_utils")]
    pub fn clear(&mut self, txn: &mut Writer) -> DatabaseResult<()> {
        let mut stmt = txn.prepare_cached(&format!("DELETE FROM {}", self.name()))?;
        let _ = stmt.execute(NO_PARAMS)?;
        Ok(())
    }
}
