//! Functionality for safely accessing LMDB database references.

use crate::{
    env::{Environment, EnvironmentKind},
    error::{DatabaseError, DatabaseResult},
};
use lazy_static::lazy_static;
use sx_types::universal_map::{Key as UmKey, UniversalMap};

use rkv::{IntegerStore, MultiStore, SingleStore, StoreOptions};

/// TODO This is incomplete
/// Enumeration of all databases needed by Holochain
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum DbName {
    /// Primary database: KV store of chain entries, keyed by address
    PrimaryChainEntries,
    /// Primary database: KV store of chain headers, keyed by address
    PrimaryChainHeaders,
    /// Primary database: KVV store of chain metadata, storing relationships
    PrimaryChainMeta,
    /// int KV store storing the sequence of committed headers,
    /// most notably allowing access to the chain head
    ChainSequence,
    /// Cache database: KV store of chain entries, keyed by address
    CacheChainEntries,
    /// Cache database: KV store of chain headers, keyed by address
    CacheChainHeaders,
    /// Cache database: KVV store of chain metadata, storing relationships
    CacheChainMeta,
    /// database which stores a single key-value pair, encoding the
    /// mutable state for the entire Conductor
    ConductorState,
    /// database that stores wasm bytecode
    Wasm,
}

impl std::fmt::Display for DbName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use DbName::*;
        match self {
            PrimaryChainEntries => write!(f, "PrimaryChainEntries"),
            PrimaryChainHeaders => write!(f, "PrimaryChainHeaders"),
            PrimaryChainMeta => write!(f, "PrimaryChainMeta"),
            ChainSequence => write!(f, "ChainSequence"),
            CacheChainEntries => write!(f, "CacheChainEntries"),
            CacheChainHeaders => write!(f, "CacheChainHeaders"),
            CacheChainMeta => write!(f, "CacheChainMeta"),
            ConductorState => write!(f, "ConductorState"),
            Wasm => write!(f, "Wasm"),
        }
    }
}

impl DbName {
    /// Associates a [DbKind] to each [DbName]
    pub fn kind(&self) -> DbKind {
        use DbKind::*;
        use DbName::*;
        match self {
            PrimaryChainEntries => Single,
            PrimaryChainHeaders => Single,
            PrimaryChainMeta => Multi,
            ChainSequence => SingleInt,
            CacheChainEntries => Single,
            CacheChainHeaders => Single,
            CacheChainMeta => Multi,
            ConductorState => Single,
            Wasm => Single,
        }
    }
}

/// The various "modes" of viewing LMDB databases
pub enum DbKind {
    /// Single-value KV with arbitrary keys, associated with [KvBuf]
    Single,
    /// Single-value KV with integer keys, associated with [IntKvBuf]
    SingleInt,
    /// Multi-value KV with arbitrary keys, associated with [KvvBuf]
    Multi,
}

/// A UniversalMap key used to access persisted database references.
/// The key type is DbName, the value can be one of the various `rkv`
/// database types
pub type DbKey<V> = UmKey<DbName, V>;

lazy_static! {
    /// The key to access the ChainEntries database
    pub static ref PRIMARY_CHAIN_ENTRIES: DbKey<SingleStore> =
    DbKey::<SingleStore>::new(DbName::PrimaryChainEntries);
    /// The key to access the ChainHeaders database
    pub static ref PRIMARY_CHAIN_HEADERS: DbKey<SingleStore> =
    DbKey::<SingleStore>::new(DbName::PrimaryChainHeaders);
    /// The key to access the ChainMeta database
    pub static ref PRIMARY_SYSTEM_META: DbKey<MultiStore> = DbKey::new(DbName::PrimaryChainMeta);
    /// The key to access the ChainMeta database
    pub static ref PRIMARY_LINKS_META: DbKey<MultiStore> = DbKey::new(DbName::PrimaryChainMeta);
    /// The key to access the ChainSequence database
    pub static ref CHAIN_SEQUENCE: DbKey<IntegerStore<u32>> = DbKey::new(DbName::ChainSequence);
    /// The key to access the ChainEntries database
    pub static ref CACHE_CHAIN_ENTRIES: DbKey<SingleStore> =
    DbKey::<SingleStore>::new(DbName::CacheChainEntries);
    /// The key to access the ChainHeaders database
    pub static ref CACHE_CHAIN_HEADERS: DbKey<SingleStore> =
    DbKey::<SingleStore>::new(DbName::CacheChainHeaders);
    /// The key to access the ChainMeta database
    pub static ref CACHE_SYSTEM_META: DbKey<MultiStore> = DbKey::new(DbName::CacheChainMeta);
    /// The key to access the ChainMeta database
    pub static ref CACHE_LINKS_META: DbKey<MultiStore> = DbKey::new(DbName::CacheChainMeta);
    /// The key to access the ConductorState database
    pub static ref CONDUCTOR_STATE: DbKey<SingleStore> = DbKey::new(DbName::ConductorState);
    /// The key to access the Wasm database
    pub static ref WASM: DbKey<SingleStore> = DbKey::new(DbName::Wasm);
}

/// DbManager is intended to be used as a singleton store for LMDB Database references,
/// so its constructor is intentionally private.
/// It uses a UniversalMap to retrieve heterogeneously typed data via special keys
/// whose type includes the type of the corresponding value.
pub struct DbManager {
    env: Environment,
    um: UniversalMap<DbName>,
}

impl DbManager {
    pub(crate) async fn new(env: Environment) -> DatabaseResult<Self> {
        let mut this = Self {
            env,
            um: UniversalMap::new(),
        };
        // TODO: rethink this. If multiple DbManagers exist for this environment, we might create DBs twice,
        // which could cause a panic.
        // This can be simplified (and made safer) if DbManager, ReadManager and WriteManager
        // are just traits of the Rkv environment.
        this.initialize().await?;
        Ok(this)
    }

    /// Get a `rkv` Database reference from a key
    pub fn get<V: 'static + Send + Sync>(&self, key: &DbKey<V>) -> DatabaseResult<&V> {
        self.um
            .get(key)
            .ok_or_else(|| DatabaseError::StoreNotInitialized(key.key().to_owned()))
    }

    async fn create<V: 'static + Send + Sync>(&mut self, key: &DbKey<V>) -> DatabaseResult<()> {
        let db_name = key.key();
        let db_str = format!("{}", db_name);
        let _ = match db_name.kind() {
            DbKind::Single => self.um.insert(
                key.with_value_type(),
                self.env
                    .inner()
                    .await
                    .open_single(db_str.as_str(), StoreOptions::create())?,
            ),
            DbKind::SingleInt => self.um.insert(
                key.with_value_type(),
                self.env
                    .inner()
                    .await
                    .open_integer::<&str, u32>(db_str.as_str(), StoreOptions::create())?,
            ),
            DbKind::Multi => self.um.insert(
                key.with_value_type(),
                self.env
                    .inner()
                    .await
                    .open_multi(db_str.as_str(), StoreOptions::create())?,
            ),
        };
        Ok(())
    }

    /// Get a `rkv` Database reference from a key, or create a new Database
    /// of the proper type if not yet created
    /*
    pub async fn get_or_create<V: 'static + Send + Sync>(
        &mut self,
        key: &DbKey<V>,
    ) -> DatabaseResult<&V> {
        if self.um.get(key).is_some() {
            Ok(self.um.get(key).unwrap())
        } else {
            self.create(key).await?;
            Ok(self.um.get(key).unwrap())
        }
    }*/

    async fn initialize(&mut self) -> DatabaseResult<()> {
        match self.env.kind() {
            EnvironmentKind::Cell(_) => {
                self.create(&*PRIMARY_CHAIN_ENTRIES).await?;
                self.create(&*PRIMARY_CHAIN_HEADERS).await?;
                self.create(&*PRIMARY_SYSTEM_META).await?;
                self.create(&*PRIMARY_LINKS_META).await?;
                self.create(&*CHAIN_SEQUENCE).await?;
                self.create(&*CACHE_CHAIN_ENTRIES).await?;
                self.create(&*CACHE_CHAIN_HEADERS).await?;
                self.create(&*CACHE_SYSTEM_META).await?;
                self.create(&*CACHE_LINKS_META).await?;
            }
            EnvironmentKind::Conductor => {
                self.create(&*CONDUCTOR_STATE).await?;
            }
            EnvironmentKind::Wasm => {
                self.create(&*WASM).await?;
            }
        }
        Ok(())
    }
}
