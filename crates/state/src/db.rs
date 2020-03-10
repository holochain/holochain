use crate::{
    env::Environment,
    error::{DatabaseError, DatabaseResult},
};
use holochain_persistence_api::univ_map::{Key as UmKey, UniversalMap};
use lazy_static::lazy_static;

use rkv::{IntegerStore, MultiStore, SingleStore, StoreOptions};

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum DbName {
    ChainEntries,
    ChainHeaders,
    ChainMeta,
    ChainSequence,
}

impl std::fmt::Display for DbName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use DbName::*;
        match self {
            ChainEntries => write!(f, "ChainEntries"),
            ChainHeaders => write!(f, "ChainHeaders"),
            ChainMeta => write!(f, "ChainMeta"),
            ChainSequence => write!(f, "ChainSequence"),
        }
    }
}

impl DbName {
    pub fn kind(&self) -> DbKind {
        use DbKind::*;
        use DbName::*;
        match self {
            ChainEntries => Single,
            ChainHeaders => Single,
            ChainMeta => Multi,
            ChainSequence => SingleInt,
        }
    }
}

pub enum DbKind {
    Single,
    SingleInt,
    Multi,
}

pub type DbKey<V> = UmKey<DbName, V>;

lazy_static! {
    pub static ref CHAIN_ENTRIES: DbKey<SingleStore> =
        DbKey::<SingleStore>::new(DbName::ChainEntries);
    pub static ref CHAIN_HEADERS: DbKey<SingleStore> =
        DbKey::<SingleStore>::new(DbName::ChainHeaders);
    pub static ref CHAIN_META: DbKey<MultiStore> = DbKey::new(DbName::ChainMeta);
    pub static ref CHAIN_SEQUENCE: DbKey<IntegerStore<u32>> = DbKey::new(DbName::ChainSequence);
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
    pub(crate) fn new(env: Environment) -> DatabaseResult<Self> {
        let mut this = Self {
            env,
            um: UniversalMap::new(),
        };
        // TODO: rethink this. If multiple DbManagers exist for this environment, we might create DBs twice,
        // which could cause a panic.
        // This can be simplified (and made safer) if DbManager, ReadManager and WriteManager
        // are just traits of the Rkv environment.
        this.initialize()?;
        Ok(this)
    }

    pub fn get<V: 'static + Send + Sync>(&self, key: &DbKey<V>) -> DatabaseResult<&V> {
        self.um
            .get(key)
            .ok_or_else(|| DatabaseError::StoreNotInitialized(key.key().to_owned()))
    }

    fn create<V: 'static + Send + Sync>(&mut self, key: &DbKey<V>) -> DatabaseResult<()> {
        let db_name = key.key();
        let db_str = format!("{}", db_name);
        let _ = match db_name.kind() {
            DbKind::Single => self.um.insert(
                key.with_value_type(),
                self.env
                    .inner()
                    .open_single(db_str.as_str(), StoreOptions::create())?,
            ),
            DbKind::SingleInt => self.um.insert(
                key.with_value_type(),
                self.env
                    .inner()
                    .open_integer::<&str, u32>(db_str.as_str(), StoreOptions::create())?,
            ),
            DbKind::Multi => self.um.insert(
                key.with_value_type(),
                self.env
                    .inner()
                    .open_multi(db_str.as_str(), StoreOptions::create())?,
            ),
        };
        Ok(())
    }

    pub fn get_or_create<V: 'static + Send + Sync>(
        &mut self,
        key: &DbKey<V>,
    ) -> DatabaseResult<&V> {
        if self.um.get(key).is_some() {
            return Ok(self.um.get(key).unwrap());
        } else {
            self.create(key)?;
            Ok(self.um.get(key).unwrap())
        }
    }

    fn initialize(&mut self) -> DatabaseResult<()> {
        self.create(&*CHAIN_ENTRIES)?;
        self.create(&*CHAIN_HEADERS)?;
        self.create(&*CHAIN_META)?;
        self.create(&*CHAIN_SEQUENCE)?;
        Ok(())
    }
}
