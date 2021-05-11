//! Helpers for unit tests

use crate::db::DbKind;
use crate::db::DbWrite;
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

#[macro_export]
/// Macro to generate a fresh reader from an DbRead with less boilerplate
/// Use this in tests, where everything gets unwrapped anyway
macro_rules! fresh_reader_test {
    ($env: expr, $f: expr) => {{
        let mut conn = $env.conn().unwrap();
        $crate::db::ReadManager::with_reader(&mut conn, |r| {
            $crate::error::DatabaseResult::Ok($f(r))
        })
        .unwrap()
    }};
}

#[macro_export]
/// Macro to generate a fresh reader from an DbRead with less boilerplate
/// Use this in tests, where everything gets unwrapped anyway
macro_rules! print_stmts_test {
    ($env: expr, $f: expr) => {{
        let mut conn = $env.conn().unwrap();
        conn.trace(Some(|s| println!("{}", s)));
        let r = $crate::db::ReadManager::with_reader(&mut conn, |r| {
            $crate::error::DatabaseResult::Ok($f(r))
        })
        .unwrap();
        conn.trace(None);
        r
    }};
}
