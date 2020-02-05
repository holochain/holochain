use crate::cell::CellId;
use holochain_persistence_api::{
    cas::content::{Address, AddressableContent},
    txn::*,
};
use holochain_persistence_lmdb::txn::*;
use lmdb::EnvironmentFlags;
use std::{
    convert::TryFrom,
    fmt::Debug,
    hash::Hash,
    path::{Path, PathBuf},
};
use sx_types::{agent::AgentId, prelude::*};

#[derive(Clone, Debug, Shrinkwrap)]
pub struct DatabasePath(PathBuf);

impl From<CellId> for DatabasePath {
    fn from((addr, id): CellId) -> Self {
        let database_path = PathBuf::new()
            .join(format!("{}", id.address()))
            .join(format!("{}", addr));
        DatabasePath(database_path.into())
    }
}

impl AsRef<Path> for DatabasePath {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

#[derive(Clone, Debug)]
pub enum LmdbSettings {
    Normal,
    Test,
}

impl From<LmdbSettings> for EnvironmentFlags {
    fn from(settings: LmdbSettings) -> EnvironmentFlags {
        match settings {
            // Note that MAP_ASYNC is absent here, because it degrades data integrity guarantees
            LmdbSettings::Normal => EnvironmentFlags::WRITE_MAP,
            LmdbSettings::Test => EnvironmentFlags::NO_SYNC,
        }
    }
}

impl Default for LmdbSettings {
    fn default() -> Self {
        LmdbSettings::Normal
    }
}

#[cfg(hey_carmelo)] // ;)
mod nice_to_have {
    pub type Cursor<A: Attribute> = <LmdbManager<A> as CursorProvider<A>>::Cursor;
    pub type CursorRw<A: Attribute> = <LmdbManager<A> as CursorProvider<A>>::CursorRw;

    pub trait TypedCursor<A: Attribute, T: TryFrom<Content>>:
        PartialEq + Eq + PartialOrd + Hash + Clone + serde::Serialize + Debug + Sync + Send
    {
        type Error: From<PersistenceError>;
        fn cursor(&self) -> &Cursor<A>;
        fn fetch(&self, address: &Address) -> Result<Option<T>, Self::Error> {
            unimplemented!()
            // self.cursor().fetch(address)?.map(|c| c.try_into())
        }
        fn contains(&self, address: &Address) -> Result<bool, Self::Error> {
            unimplemented!()
        }

        // and so on...
    }
}
