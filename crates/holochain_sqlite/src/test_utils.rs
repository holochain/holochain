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
    let tmpdir = TempDir::new("holochain-test-environments").unwrap();
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
#[derive(Shrinkwrap)]
pub struct TestDb {
    #[shrinkwrap(main_field)]
    /// sqlite database
    db: DbWrite,
    /// temp directory for this environment
    tmpdir: TempDir,
}

impl TestDb {
    /// Accessor
    pub fn db(&self) -> DbWrite {
        self.db.clone()
    }

    /// Accessor
    pub fn into_tempdir(self) -> TempDir {
        self.tmpdir
    }
}

/// A container for all three non-cell environments
pub struct TestDbs {
    /// A test conductor environment
    conductor: DbWrite,
    /// A test wasm environment
    wasm: DbWrite,
    /// A test p2p state environment
    p2p_agent_store: DbWrite,
    /// A test p2p metrics environment
    p2p_metrics: DbWrite,
    /// The shared root temp dir for these environments
    tempdir: TempDir,
}

#[allow(missing_docs)]
impl TestDbs {
    /// Create all three non-cell environments at once
    pub fn new(tempdir: TempDir) -> Self {
        use DbKind::*;
        let conductor = DbWrite::new(&tempdir.path(), Conductor).unwrap();
        let wasm = DbWrite::new(&tempdir.path(), Wasm).unwrap();
        let space = Arc::new(kitsune_p2p::KitsuneSpace(vec![0; 36]));
        let p2p_agent_store = DbWrite::new(&tempdir.path(), P2pAgentStore(space.clone())).unwrap();
        let p2p_metrics = DbWrite::new(&tempdir.path(), P2pMetrics(space)).unwrap();
        Self {
            conductor,
            wasm,
            p2p_agent_store,
            p2p_metrics,
            tempdir,
        }
    }

    pub fn conductor(&self) -> DbWrite {
        self.conductor.clone()
    }

    pub fn wasm(&self) -> DbWrite {
        self.wasm.clone()
    }

    pub fn p2p_agent_store(&self) -> DbWrite {
        self.p2p_agent_store.clone()
    }

    pub fn p2p_metrics(&self) -> DbWrite {
        self.p2p_metrics.clone()
    }

    /// Get the root temp dir for these environments
    pub fn into_tempdir(self) -> TempDir {
        self.tempdir
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
