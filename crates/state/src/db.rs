use crate::error::WorkspaceResult;
use holochain_persistence_api::univ_map::{Key as UmKey, UniversalMap};
use owning_ref::ArcRef;
use rkv::{Rkv, StoreOptions};
use std::sync::{Arc, RwLock};
use sx_types::{agent::CellId, prelude::AddressableContent};

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

pub struct DbManager {
    // NOTE: this can't just be an Rkv because we get Rkv environments from the Manager
    // already wrapped in the Arc<RwLock<_>>, so this is the canonical representation of an LMDB environment
    env: Arc<RwLock<Rkv>>,
    um: UniversalMap<DbName>,
}

impl DbManager {
    pub fn new(env: Arc<RwLock<Rkv>>) -> Self {
        Self {
            env,
            um: UniversalMap::new(),
        }
    }

    pub fn get<V: 'static + Send + Sync>(&self, key: &DbKey<V>) -> WorkspaceResult<&V> {
        Ok(self.um.get(&key).unwrap())
    }

    pub fn get_or_insert<V: 'static + Send + Sync>(&mut self, key: DbKey<V>) -> WorkspaceResult<&V> {
        if self.um.get(&key).is_some() {
            return Ok(self.um.get(&key).unwrap());
        } else {
            let env = self.env.read().unwrap();
            let db_name = key.key();
            let db_str = format!("{}", db_name);
            let _ = match db_name.kind() {
                DbKind::Single => self.um.insert(
                    key.with_value_type(),
                    env.open_single(db_str.as_str(), StoreOptions::create())?,
                ),
                DbKind::SingleInt => self.um.insert(
                    key.with_value_type(),
                    env.open_integer::<&str, u32>(db_str.as_str(), StoreOptions::create())?,
                ),
                DbKind::Multi => self.um.insert(
                    key.with_value_type(),
                    env.open_multi(db_str.as_str(), StoreOptions::create())?,
                ),
            };
            Ok(self.um.get(&key).unwrap().clone())
        }
    }
}
