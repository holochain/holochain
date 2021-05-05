//! Functionality for safely accessing LMDB database references.

use crate::env::EnvironmentKind;
use crate::error::DatabaseError;
use crate::error::DatabaseResult;
use crate::exports::IntegerStore;
use crate::prelude::IntKey;
use crate::universal_map::Key as UmKey;
use crate::universal_map::UniversalMap;
use derive_more::Display;
use holochain_keystore::KeystoreSender;
use lazy_static::lazy_static;
use parking_lot::RwLock;
use rkv::MultiStore;
use rkv::Rkv;
use rkv::SingleStore;
use rkv::StoreOptions;
use std::collections::hash_map;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

/// TODO This is incomplete
/// Enumeration of all databases needed by Holochain
#[derive(Clone, Debug, Hash, PartialEq, Eq, Display)]
pub enum DbName {
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

impl DbName {
    /// Associates a [DbKind] to each [DbName]
    pub fn kind(&self) -> DbKind {
        use DbKind::*;
        use DbName::*;
        match self {
            ElementVaultPublicEntries => Single,
            ElementVaultPrivateEntries => Single,
            ElementVaultHeaders => Single,
            MetaVaultSys => Multi,
            MetaVaultLinks => Single,
            MetaVaultMisc => Single,
            ChainSequence => SingleInt,
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
        }
    }
}

#[derive(Debug)]
/// The various "modes" of viewing LMDB databases
pub enum DbKind {
    /// Single-value KV with arbitrary keys, associated with [KvBufFresh]
    Single,
    /// Single-value KV with integer keys, associated with [KvIntBufFresh]
    SingleInt,
    /// Multi-value KV with arbitrary keys, associated with [KvvBufUsed]
    Multi,
}

/// A UniversalMap key used to access persisted database references.
/// The key type is DbName, the value can be one of the various `rkv`
/// database types
pub type DbKey<V> = UmKey<DbName, V>;

type DbMap = UniversalMap<DbName>;

lazy_static! {
    /// The key to access the ChainEntries database
    pub static ref ELEMENT_VAULT_PUBLIC_ENTRIES: DbKey<SingleStore> =
    DbKey::<SingleStore>::new(DbName::ElementVaultPublicEntries);
    /// The key to access the PrivateChainEntries database
    pub static ref ELEMENT_VAULT_PRIVATE_ENTRIES: DbKey<SingleStore> =
    DbKey::<SingleStore>::new(DbName::ElementVaultPrivateEntries);
    /// The key to access the ChainHeaders database
    pub static ref ELEMENT_VAULT_HEADERS: DbKey<SingleStore> =
    DbKey::<SingleStore>::new(DbName::ElementVaultHeaders);
    /// The key to access the Metadata database of the Vault
    pub static ref META_VAULT_SYS: DbKey<MultiStore> = DbKey::new(DbName::MetaVaultSys);
    /// The key to access the links database of the Vault
    pub static ref META_VAULT_LINKS: DbKey<SingleStore> = DbKey::new(DbName::MetaVaultLinks);
    /// The key to access the miscellaneous metadata database of the Vault
    pub static ref META_VAULT_MISC: DbKey<SingleStore> = DbKey::new(DbName::MetaVaultMisc);
    /// The key to access the ChainSequence database
    pub static ref CHAIN_SEQUENCE: DbKey<IntegerStore> = DbKey::new(DbName::ChainSequence);
    /// The key to access the ChainEntries database
    pub static ref ELEMENT_CACHE_ENTRIES: DbKey<SingleStore> =
    DbKey::<SingleStore>::new(DbName::ElementCacheEntries);
    /// The key to access the ChainHeaders database
    pub static ref ELEMENT_CACHE_HEADERS: DbKey<SingleStore> =
    DbKey::<SingleStore>::new(DbName::ElementCacheHeaders);
    /// The key to access the Metadata database of the Cache
    pub static ref CACHE_SYSTEM_META: DbKey<MultiStore> = DbKey::new(DbName::MetaCacheSys);
    /// The key to access the links database of the Cache
    pub static ref CACHE_LINKS_META: DbKey<SingleStore> = DbKey::new(DbName::MetaCacheLinks);
    /// The key to access the status database of the Cache
    pub static ref CACHE_STATUS_META: DbKey<SingleStore> = DbKey::new(DbName::MetaCacheStatus);
    /// The key to access the ConductorState database
    pub static ref CONDUCTOR_STATE: DbKey<SingleStore> = DbKey::new(DbName::ConductorState);
    /// The key to access the Wasm database
    pub static ref WASM: DbKey<SingleStore> = DbKey::new(DbName::Wasm);
    /// The key to access the DnaDef database
    pub static ref DNA_DEF: DbKey<SingleStore> = DbKey::new(DbName::DnaDef);
    /// The key to access the EntryDef database
    pub static ref ENTRY_DEF: DbKey<SingleStore> = DbKey::new(DbName::EntryDef);
    /// The key to access the AuthoredDhtOps database
    pub static ref AUTHORED_DHT_OPS: DbKey<SingleStore> = DbKey::new(DbName::AuthoredDhtOps);
    /// The key to access the IntegratedDhtOps database
    pub static ref INTEGRATED_DHT_OPS: DbKey<SingleStore> = DbKey::new(DbName::IntegratedDhtOps);
    /// The key to access the IntegrationLimbo database
    pub static ref INTEGRATION_LIMBO: DbKey<SingleStore> = DbKey::new(DbName::IntegrationLimbo);
    /// The key to access the IntegrationLimbo database
    pub static ref VALIDATION_LIMBO: DbKey<SingleStore> = DbKey::new(DbName::ValidationLimbo);
    /// The key to access the ValidationReceipts database
    pub static ref VALIDATION_RECEIPTS: DbKey<MultiStore> = DbKey::new(DbName::ValidationReceipts);
    /// The key to access the Agent database
    pub static ref AGENT: DbKey<SingleStore> = DbKey::new(DbName::Agent);
}

lazy_static! {
    static ref DB_MAP_MAP: RwLock<HashMap<PathBuf, DbMap>> = RwLock::new(HashMap::new());
}

/// Get access to the singleton database manager ([GetDb]),
/// in order to access individual LMDB databases
pub(super) fn initialize_databases(rkv: &Rkv, kind: &EnvironmentKind) -> DatabaseResult<()> {
    let mut dbmap = DB_MAP_MAP.write();
    let path = rkv.path().to_owned();
    match dbmap.entry(path.clone()) {
        hash_map::Entry::Occupied(_) => {
            tracing::warn!(
                "Ignored attempt to double-initialize an LMDB environment: {:?}",
                path
            );
        }
        hash_map::Entry::Vacant(e) => {
            e.insert({
                let mut um = UniversalMap::new();
                register_databases(&rkv, kind, &mut um)?;
                um
            });
        }
    };
    Ok(())
}

pub(super) fn get_db<V: 'static + Copy + Send + Sync>(
    path: &Path,
    key: &'static DbKey<V>,
) -> DatabaseResult<V> {
    let dbmap = DB_MAP_MAP.read();
    let um: &DbMap = dbmap
        .get(path)
        .ok_or_else(|| DatabaseError::EnvironmentMissing(path.into()))?;
    let db = *um
        .get(key)
        .ok_or_else(|| DatabaseError::StoreNotInitialized(key.key().clone(), path.to_owned()))?;
    Ok(db)
}

fn register_databases(env: &Rkv, kind: &EnvironmentKind, um: &mut DbMap) -> DatabaseResult<()> {
    match kind {
        EnvironmentKind::Cell(_) => {
            register_db(env, um, &*ELEMENT_VAULT_PUBLIC_ENTRIES)?;
            register_db(env, um, &*ELEMENT_VAULT_PRIVATE_ENTRIES)?;
            register_db(env, um, &*ELEMENT_VAULT_HEADERS)?;
            register_db(env, um, &*META_VAULT_SYS)?;
            register_db(env, um, &*META_VAULT_LINKS)?;
            register_db(env, um, &*META_VAULT_MISC)?;
            register_db(env, um, &*CHAIN_SEQUENCE)?;
            register_db(env, um, &*ELEMENT_CACHE_ENTRIES)?;
            register_db(env, um, &*ELEMENT_CACHE_HEADERS)?;
            register_db(env, um, &*CACHE_SYSTEM_META)?;
            register_db(env, um, &*CACHE_LINKS_META)?;
            register_db(env, um, &*CACHE_STATUS_META)?;
            register_db(env, um, &*AUTHORED_DHT_OPS)?;
            register_db(env, um, &*INTEGRATED_DHT_OPS)?;
            register_db(env, um, &*INTEGRATION_LIMBO)?;
            register_db(env, um, &*VALIDATION_LIMBO)?;
            register_db(env, um, &*VALIDATION_RECEIPTS)?;
        }
        EnvironmentKind::Conductor => {
            register_db(env, um, &*CONDUCTOR_STATE)?;
        }
        EnvironmentKind::Wasm => {
            register_db(env, um, &*WASM)?;
            register_db(env, um, &*DNA_DEF)?;
            register_db(env, um, &*ENTRY_DEF)?;
        }
        EnvironmentKind::P2p => {
            register_db(env, um, &*AGENT)?;
            // @todo health metrics for the space
            // register_db(env, um, &*HEALTH)?;
        }
    }
    Ok(())
}

fn register_db<V: 'static + Send + Sync>(
    env: &Rkv,
    um: &mut DbMap,
    key: &DbKey<V>,
) -> DatabaseResult<()> {
    let db_name = key.key();
    let db_str = format!("{}", db_name);
    let _ = match db_name.kind() {
        DbKind::Single => um.insert(
            key.with_value_type(),
            env.open_single(db_str.as_str(), StoreOptions::create())?,
        ),
        DbKind::SingleInt => um.insert(
            key.with_value_type(),
            env.open_integer::<&str, IntKey>(db_str.as_str(), StoreOptions::create())?,
        ),
        DbKind::Multi => {
            let mut opts = StoreOptions::create();

            // This is needed for the optional put flag NO_DUP_DATA on KvvBufUsed.
            // As far as I can tell, if we are not using NO_DUP_DATA, it will
            // only affect the sorting of the values in case there are dups,
            // which should be ok for our usage.
            //
            // NOTE - see:
            // https://github.com/mozilla/rkv/blob/0.10.4/src/env.rs#L122-L131
            //
            // Aparently RKV already sets this flag, but it's not mentioned
            // in the docs anywhere. We're going to set it too, just in case
            // it is removed out from under us at some point in the future.
            opts.flags.set(rkv::DatabaseFlags::DUP_SORT, true);

            um.insert(
                key.with_value_type(),
                env.open_multi(db_str.as_str(), opts)?,
            )
        }
    };
    Ok(())
}

/// GetDb allows access to the UniversalMap which stores the heterogeneously typed
/// LMDB Database references.
pub trait GetDb {
    /// Access an LMDB database environment stored in our static registrar.
    fn get_db<V: 'static + Copy + Send + Sync>(&self, key: &'static DbKey<V>) -> DatabaseResult<V>;
    /// Get a KeystoreSender to communicate with the Keystore task for this environment
    fn keystore(&self) -> KeystoreSender;
}
