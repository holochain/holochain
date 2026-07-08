//! Helpers for unit tests

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use holochain_keystore::MetaLairClient;
use holochain_sqlite::rusqlite::Statement;
use holochain_sqlite::rusqlite::Transaction;
use holochain_types::prelude::*;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

pub mod mutations_helpers;

/// Create an in-memory [`crate::dht_store::DhtStore`] for use in tests.
///
/// The underlying database is ephemeral and will be lost when the store is dropped.
pub async fn test_dht_store(dna_hash: DnaHash) -> crate::dht_store::DhtStore {
    let db = holochain_data::test_open_db(holochain_data::kind::Dht::new(Arc::new(dna_hash)))
        .await
        .expect("Failed to open test DHT database");
    crate::dht_store::DhtStore::new(db)
}

/// Create a fresh set of test environments with a new TempDir
pub fn test_db_dir() -> TempDir {
    tempfile::Builder::new()
        .prefix("holochain-test-environments")
        .suffix(&nanoid::nanoid!())
        .tempdir()
        .unwrap()
}

/// Create a fresh set of test environments with a new TempDir in a given directory.
pub fn test_dbs_in(path: impl AsRef<Path>) -> TestDbs {
    let tempdir = tempfile::Builder::new()
        .prefix("holochain-test-environments")
        .suffix(&nanoid::nanoid!())
        .tempdir_in(path)
        .unwrap();
    TestDbs::new(tempdir)
}

/// A container for all three non-cell environments
pub struct TestDbs {
    /// The shared root temp dir for these environments
    dir: TestDir,
    /// The keystore sender for these environments
    keystore: MetaLairClient,
}

#[derive(Debug)]
pub enum TestDir {
    Temp(TempDir),
    Perm(PathBuf),
    Blank,
}

impl AsRef<Path> for TestDir {
    fn as_ref(&self) -> &Path {
        match self {
            Self::Temp(d) => d.path(),
            Self::Perm(d) => d.as_path(),
            Self::Blank => unreachable!(),
        }
    }
}

impl std::ops::Deref for TestDir {
    type Target = Path;

    fn deref(&self) -> &Path {
        match self {
            Self::Temp(d) => d.path(),
            Self::Perm(d) => d.as_path(),
            Self::Blank => unreachable!(),
        }
    }
}

impl From<TempDir> for TestDir {
    fn from(d: TempDir) -> Self {
        Self::new(d)
    }
}

impl TestDir {
    pub fn new(d: TempDir) -> Self {
        Self::Temp(d)
    }

    pub fn persist(&mut self) {
        let old = std::mem::replace(self, Self::Blank);
        match old {
            Self::Temp(d) => {
                println!("Made temp dir permanent at {d:?}");
                tracing::info!("Made temp dir permanent at {:?}", d);
                *self = Self::Perm(d.keep());
            }
            old => *self = old,
        }
    }
}

#[allow(missing_docs)]
impl TestDbs {
    /// Create all four non-cell environments at once with a custom keystore
    pub fn with_keystore(tempdir: TempDir, keystore: MetaLairClient) -> Self {
        Self {
            dir: TestDir::new(tempdir),
            keystore,
        }
    }

    /// Create all three non-cell environments at once with a test keystore
    pub fn new(tempdir: TempDir) -> Self {
        Self::with_keystore(tempdir, holochain_keystore::test_keystore())
    }

    /// Get the root path for these environments
    pub fn path(&self) -> &Path {
        &self.dir
    }

    pub fn keystore(&self) -> &MetaLairClient {
        &self.keystore
    }
}

/// Produce file and line number info at compile-time
#[macro_export]
macro_rules! here {
    ($test: expr) => {
        concat!($test, " !!!_LOOK HERE:---> ", file!(), ":", line!())
    };
}

#[cfg_attr(feature = "instrument", tracing::instrument(skip(txn)))]
pub fn dump_db(txn: &Transaction) {
    let dump = |mut stmt: Statement| {
        let mut rows = stmt.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            for column in row.as_ref().column_names() {
                let row = row.get_ref_unwrap(column);
                match row {
                    holochain_sqlite::rusqlite::types::ValueRef::Null
                    | holochain_sqlite::rusqlite::types::ValueRef::Integer(_)
                    | holochain_sqlite::rusqlite::types::ValueRef::Real(_) => {
                        tracing::debug!(?column, ?row);
                    }
                    holochain_sqlite::rusqlite::types::ValueRef::Text(text) => {
                        tracing::debug!(?column, row = ?String::from_utf8_lossy(text));
                    }
                    holochain_sqlite::rusqlite::types::ValueRef::Blob(blob) => {
                        let blob = URL_SAFE_NO_PAD.encode(blob);
                        tracing::debug!("column: {:?} row:{}", column, blob);
                    }
                }
            }
        }
    };
    tracing::debug!("Actions:");
    let stmt = txn.prepare("SELECT * FROM Action").unwrap();
    dump(stmt);

    tracing::debug!("Entries:");
    let stmt = txn.prepare("SELECT * FROM Entry").unwrap();
    dump(stmt);

    tracing::debug!("DhtOps:");
    let stmt = txn.prepare("SELECT * FROM DhtOp").unwrap();
    dump(stmt);
}
