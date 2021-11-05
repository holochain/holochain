//! Helpers for unit tests

use crate::conn::DbSyncLevel;
use crate::db::DbKindAuthored;
use crate::db::DbKindConductor;
use crate::db::DbKindP2pAgentStore;
use crate::db::DbKindP2pMetrics;
use crate::db::DbKindT;
use crate::db::DbKindWasm;
use crate::db::DbWrite;
use holochain_zome_types::fake_dna_hash;
use shrinkwraprs::Shrinkwrap;
use std::sync::Arc;
use tempdir::TempDir;

/// Create a [TestDb] of [`DbKindAuthored`], backed by a temp directory.
pub fn test_authored_db() -> TestDb<DbKindAuthored> {
    let dna_hash = fake_dna_hash(1);
    test_db(DbKindAuthored(Arc::new(dna_hash)))
}

fn test_db<Kind: DbKindT + Send + Sync + 'static>(kind: Kind) -> TestDb<Kind> {
    let tmpdir = TempDir::new("holochain-test-environments").unwrap();
    TestDb {
        db: DbWrite::new(tmpdir.path(), kind, crate::conn::DbSyncLevel::default())
            .expect("Couldn't create test database"),
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
pub struct TestDb<Kind: DbKindT> {
    #[shrinkwrap(main_field)]
    /// sqlite database
    db: DbWrite<Kind>,
    /// temp directory for this environment
    tmpdir: TempDir,
}

impl<Kind: DbKindT> TestDb<Kind> {
    /// Accessor
    pub fn db(&self) -> DbWrite<Kind> {
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
    conductor: DbWrite<DbKindConductor>,
    /// A test wasm environment
    wasm: DbWrite<DbKindWasm>,
    /// A test p2p state environment
    p2p_agent_store: DbWrite<DbKindP2pAgentStore>,
    /// A test p2p metrics environment
    p2p_metrics: DbWrite<DbKindP2pMetrics>,
    /// The shared root temp dir for these environments
    tempdir: TempDir,
}

#[allow(missing_docs)]
impl TestDbs {
    /// Create all three non-cell environments at once
    pub fn new(tempdir: TempDir) -> Self {
        let conductor =
            DbWrite::new(tempdir.path(), DbKindConductor, DbSyncLevel::default()).unwrap();
        let wasm = DbWrite::new(tempdir.path(), DbKindWasm, DbSyncLevel::default()).unwrap();
        let space = Arc::new(kitsune_p2p::KitsuneSpace(vec![0; 36]));
        let p2p_agent_store = DbWrite::new(
            tempdir.path(),
            DbKindP2pAgentStore(space.clone()),
            DbSyncLevel::default(),
        )
        .unwrap();
        let p2p_metrics = DbWrite::new(
            tempdir.path(),
            DbKindP2pMetrics(space),
            DbSyncLevel::default(),
        )
        .unwrap();
        Self {
            conductor,
            wasm,
            p2p_agent_store,
            p2p_metrics,
            tempdir,
        }
    }

    pub fn conductor(&self) -> DbWrite<DbKindConductor> {
        self.conductor.clone()
    }

    pub fn wasm(&self) -> DbWrite<DbKindWasm> {
        self.wasm.clone()
    }

    pub fn p2p_agent_store(&self) -> DbWrite<DbKindP2pAgentStore> {
        self.p2p_agent_store.clone()
    }

    pub fn p2p_metrics(&self) -> DbWrite<DbKindP2pMetrics> {
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
