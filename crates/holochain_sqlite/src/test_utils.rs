//! Helpers for unit tests

use crate::db::DbKind;
use crate::db::DbWrite;
use crate::prelude::BufKey;
use holochain_zome_types::test_utils::fake_cell_id;
use shrinkwraprs::Shrinkwrap;
use std::sync::Arc;
use tempdir::TempDir;

/// Create a [TestDb] of [DbKind::Cell], backed by a temp directory.
pub fn test_cell_db() -> TestDb {
    let cell_id = fake_cell_id(1);
    test_db(DbKind::Cell(cell_id))
}

fn test_db(kind: DbKind) -> TestDb {
    let tmpdir = Arc::new(TempDir::new("holochain-test-environments").unwrap());
    TestDb {
        db: DbWrite::new(tmpdir.path(), kind).expect("Couldn't create test database"),
        tmpdir,
    }
}

/// Create a fresh set of test environments with a new TempDir
pub fn test_dbs() -> TestDbs {
    let tempdir = TempDir::new("holochain-test-environments").unwrap();
    TestDbs::new(tempdir)
}

/// A test database in a temp directory
#[derive(Clone, Shrinkwrap)]
pub struct TestDb {
    #[shrinkwrap(main_field)]
    /// sqlite database
    db: DbWrite,
    /// temp directory for this environment
    tmpdir: Arc<TempDir>,
}

impl TestDb {
    /// Accessor
    pub fn db(&self) -> DbWrite {
        self.db.clone()
    }

    /// Accessor
    pub fn tmpdir(&self) -> Arc<TempDir> {
        self.tmpdir.clone()
    }
}

#[derive(Clone)]
/// A container for all three non-cell environments
pub struct TestDbs {
    /// A test conductor environment
    conductor: DbWrite,
    /// A test wasm environment
    wasm: DbWrite,
    /// A test p2p environment
    p2p: DbWrite,
    /// The shared root temp dir for these environments
    tempdir: Arc<TempDir>,
}

#[allow(missing_docs)]
impl TestDbs {
    /// Create all three non-cell environments at once
    pub fn new(tempdir: TempDir) -> Self {
        use DbKind::*;
        let conductor = DbWrite::new(&tempdir.path(), Conductor).unwrap();
        let wasm = DbWrite::new(&tempdir.path(), Wasm).unwrap();
        let p2p = DbWrite::new(&tempdir.path(), P2p).unwrap();
        Self {
            conductor,
            wasm,
            p2p,
            tempdir: Arc::new(tempdir),
        }
    }

    pub fn conductor(&self) -> DbWrite {
        self.conductor.clone()
    }

    pub fn wasm(&self) -> DbWrite {
        self.wasm.clone()
    }

    pub fn p2p(&self) -> DbWrite {
        self.p2p.clone()
    }

    /// Get the root temp dir for these environments
    pub fn tempdir(&self) -> Arc<TempDir> {
        self.tempdir.clone()
    }
}

/// A String-based newtype suitable for database keys and values
#[derive(
    Clone,
    Debug,
    PartialOrd,
    Ord,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    derive_more::Display,
    derive_more::From,
)]
pub struct DbString(String);

impl AsRef<[u8]> for DbString {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl rusqlite::ToSql for DbString {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Borrowed(self.as_ref().into()))
    }
}

impl BufKey for DbString {
    fn from_key_bytes_or_friendly_panic(bytes: &[u8]) -> Self {
        Self(String::from_utf8(bytes.to_vec()).unwrap())
    }
}

impl From<&str> for DbString {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}
