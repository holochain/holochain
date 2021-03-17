#![allow(deprecated)]
#![allow(dead_code)]

use crate::{prelude::*, swansong::SwanSong};
use lazy_static::lazy_static;
use parking_lot::{Mutex, MutexGuard, RwLock};
use rusqlite::*;
use std::{
    collections::{hash_map::Entry, HashMap},
    path::{Path, PathBuf},
    sync::Arc,
};

use super::initialize_connection;

lazy_static! {
    pub(crate) static ref CONNECTIONS: RwLock<HashMap<PathBuf, SConn>> =
        RwLock::new(HashMap::new());
}

/// Singleton Connection.
/// We went with Pooled connections for now, but leaving this here in case we
/// want to go back to singletons at some point.
#[deprecated = "remove if we never wind up using singleton connections"]
#[derive(Clone)]
pub struct SConn {
    inner: Arc<Mutex<Connection>>,
    kind: DbKind,
}

impl SConn {
    /// Create a new connection with decryption key set
    pub fn open(path: &Path, kind: &DbKind) -> DatabaseResult<Self> {
        let mut conn = Connection::open(path)?;
        initialize_connection(&mut conn, kind, true)?;
        Ok(Self::new(conn, kind.clone()))
    }

    fn new(inner: Connection, kind: DbKind) -> Self {
        Self {
            inner: Arc::new(Mutex::new(inner)),
            kind,
        }
    }

    pub fn inner(&mut self) -> SwanSong<MutexGuard<Connection>> {
        let kind = self.kind.clone();
        tracing::trace!("lock attempt {}", &kind);
        let guard = self
            .inner
            .try_lock_for(std::time::Duration::from_secs(30))
            .unwrap_or_else(|| panic!(format!("Couldn't unlock connection. Kind: {}", &kind)));
        tracing::trace!("lock success {}", &kind);
        SwanSong::new(guard, move |_| {
            tracing::trace!("lock drop {}", &kind);
        })
    }

    #[cfg(feature = "test_utils")]
    pub fn open_single(&mut self, name: &str) -> Result<SingleTable, DatabaseError> {
        crate::table::initialize_table_single(
            &mut self.inner(),
            name.to_string(),
            name.to_string(),
        )?;
        Ok(Table {
            name: TableName::TestSingle(name.to_string()),
        })
    }

    #[cfg(feature = "test_utils")]
    pub fn open_integer(&mut self, name: &str) -> Result<IntegerTable, DatabaseError> {
        self.open_single(name)
    }

    #[cfg(feature = "test_utils")]
    pub fn open_multi(&mut self, name: &str) -> Result<MultiTable, DatabaseError> {
        crate::table::initialize_table_multi(
            &mut self.inner(),
            name.to_string(),
            name.to_string(),
        )?;
        Ok(Table {
            name: TableName::TestMulti(name.to_string()),
        })
    }
}

impl DbRead {
    #[deprecated = "remove if we never use singleton connections"]
    fn _connection_naive(&self) -> DatabaseResult<SConn> {
        Ok(SConn::open(self.path(), &self.kind())?)
    }

    #[deprecated = "remove if we never use singleton connections"]
    fn _connection_singleton(&self) -> DatabaseResult<SConn> {
        let mut map = CONNECTIONS.write();
        let conn = match map.entry(self.path().to_owned()) {
            Entry::Vacant(e) => {
                let conn = SConn::open(self.path(), self.kind())?;
                e.insert(conn).clone()
            }
            Entry::Occupied(e) => e.get().clone(),
        };

        Ok(conn)
    }
}

// impl<'e> ReadManager<'e> for SConn {
//     fn with_reader<E, R, F>(&'e mut self, f: F) -> Result<R, E>
//     where
//         E: From<DatabaseError>,
//         F: 'e + FnOnce(Reader) -> Result<R, E>,
//     {
//         let mut g = self.inner();
//         let txn = g.transaction().map_err(DatabaseError::from)?;
//         let reader = Reader::from(txn);
//         f(reader)
//     }

//     #[cfg(feature = "test_utils")]
//     fn with_reader_test<R, F>(&'e mut self, f: F) -> R
//     where
//         F: 'e + FnOnce(Reader) -> R,
//     {
//         self.with_reader(|r| DatabaseResult::Ok(f(r))).unwrap()
//     }
// }

// impl<'e> WriteManager<'e> for SConn {
//     fn with_commit<E, R, F>(&'e mut self, f: F) -> Result<R, E>
//     where
//         E: From<DatabaseError>,
//         F: 'e + FnOnce(&mut Writer) -> Result<R, E>,
//     {
//         let mut b = self.inner();
//         let txn = b.transaction().map_err(DatabaseError::from)?;
//         let mut writer = txn;
//         let result = f(&mut writer)?;
//         writer.commit().map_err(DatabaseError::from)?;
//         Ok(result)
//     }
// }
