use crate::{
    error::{WorkspaceError, WorkspaceResult},
    Reader, Writer, env::Env,
};
use holochain_persistence_api::univ_map::{Key as UmKey, UniversalMap};
use lazy_static::lazy_static;

use rkv::{IntegerStore, MultiStore, Rkv, SingleStore, StoreOptions};



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

pub struct DbManager<'env> {
    // NOTE: this can't just be an Rkv because we get Rkv environments from the Manager
    // already wrapped in the Arc<RwLock<_>>, so this is the canonical representation of an LMDB environment
    env: Env<'env>,
    um: UniversalMap<DbName>,
}

impl<'env> DbManager<'env> {
    pub fn new(env: Env<'env>) -> WorkspaceResult<Self> {
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

    pub fn get<V: 'static + Send + Sync>(&self, key: &DbKey<V>) -> WorkspaceResult<&V> {
        self.um
            .get(key)
            .ok_or(WorkspaceError::StoreNotInitialized(key.key().to_owned()))
    }

    fn create<V: 'static + Send + Sync>(&mut self, key: &DbKey<V>) -> WorkspaceResult<()> {
        let db_name = key.key();
        let db_str = format!("{}", db_name);
        let _ = match db_name.kind() {
            DbKind::Single => self.um.insert(
                key.with_value_type(),
                self.env.inner().open_single(db_str.as_str(), StoreOptions::create())?,
            ),
            DbKind::SingleInt => self.um.insert(
                key.with_value_type(),
                self.env.inner().open_integer::<&str, u32>(db_str.as_str(), StoreOptions::create())?,
            ),
            DbKind::Multi => self.um.insert(
                key.with_value_type(),
                self.env.inner().open_multi(db_str.as_str(), StoreOptions::create())?,
            ),
        };
        Ok(())
    }

    fn initialize(&mut self) -> WorkspaceResult<()> {
        self.create(&*CHAIN_ENTRIES)?;
        self.create(&*CHAIN_HEADERS)?;
        self.create(&*CHAIN_META)?;
        self.create(&*CHAIN_SEQUENCE)?;
        Ok(())
    }

    pub fn get_or_create<V: 'static + Send + Sync>(
        &mut self,
        key: &DbKey<V>,
    ) -> WorkspaceResult<&V> {
        if self.um.get(key).is_some() {
            return Ok(self.um.get(key).unwrap());
        } else {
            self.create(key)?;
            Ok(self.um.get(key).unwrap().clone())
        }
    }
}

// pub struct ReadManager<'env>(&'env Rkv);

// impl<'e> ReadManager<'e> {
//     pub fn new(env: &'e Rkv) -> Self {
//         Self(env)
//     }

//     pub fn reader(&self) -> WorkspaceResult<Reader<'e>> {
//         Ok(Reader(self.0.read()?))
//     }

//     pub fn with_reader<R, F: FnOnce(Reader) -> WorkspaceResult<R>>(
//         &self,
//         f: F,
//     ) -> WorkspaceResult<R> {
//         f(Reader(self.0.read()?))
//     }
// }

// pub struct WriteManager<'env>(&'env Rkv);

// impl<'e> WriteManager<'e> {
//     pub fn new(env: &'e Rkv) -> Self {
//         Self(env)
//     }

//     pub fn with_commit<R, F: FnOnce(&mut Writer) -> WorkspaceResult<R>>(
//         &self,
//         f: F,
//     ) -> WorkspaceResult<R> {
//         let mut writer = self.0.write()?;
//         let result = f(&mut writer);
//         writer.commit()?;
//         result
//     }
// }
