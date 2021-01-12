//! Functionality for safely accessing LMDB database references.

use crate::env::EnvironmentKind;
use crate::error::DatabaseResult;
use derive_more::Display;
use rkv::Rkv;
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

/// Get access to the singleton database manager ([GetDb]),
/// in order to access individual LMDB databases
pub(super) fn initialize_databases(rkv: &Rkv, kind: &EnvironmentKind) -> DatabaseResult<()> {
    todo!("create database and schema if not exists");
    Ok(())
}
