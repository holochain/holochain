//! Helpers for unit tests

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use holochain_keystore::MetaLairClient;
use holochain_sqlite::prelude::*;
use holochain_sqlite::rusqlite::Statement;
use holochain_sqlite::rusqlite::Transaction;
use holochain_types::prelude::*;
use holochain_zome_types::test_utils::fake_cell_id;
use shrinkwraprs::Shrinkwrap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

pub mod mutations_helpers;

/// Create a [`TestDb`] of [`DbKindAuthored`], backed by a temp directory.
pub fn test_authored_db() -> TestDb<DbKindAuthored> {
    test_authored_db_with_id(1)
}

/// Create a test authored database with a DNA hash and agent key based on the input `id`.
pub fn test_authored_db_with_id(id: u8) -> TestDb<DbKindAuthored> {
    test_db(DbKindAuthored(Arc::new(CellId::new(
        fake_dna_hash(id),
        fake_agent_pub_key(id),
    ))))
}

/// Create a [`TestDb`] of [`DbKindDht`], backed by a temp directory.
pub fn test_dht_db() -> TestDb<DbKindDht> {
    test_dht_db_with_id(1)
}

pub fn test_dht_db_with_id(id: u8) -> TestDb<DbKindDht> {
    test_db(DbKindDht(Arc::new(fake_dna_hash(id))))
}

pub fn test_dht_db_with_dna_hash(hash: DnaHash) -> TestDb<DbKindDht> {
    test_db(DbKindDht(Arc::new(hash)))
}

/// Create a [`TestDb`] of [`DbKindCache`], backed by a temp directory.
pub fn test_cache_db() -> TestDb<DbKindCache> {
    test_cache_db_with_id(1)
}

pub fn test_cache_db_with_id(id: u8) -> TestDb<DbKindCache> {
    test_db(DbKindCache(Arc::new(fake_cell_id(id).dna_hash().clone())))
}

pub fn test_cache_db_with_dna_hash(hash: DnaHash) -> TestDb<DbKindCache> {
    test_db(DbKindCache(Arc::new(hash)))
}

/// Create a [`TestDb`] of [DbKindConductor], backed by a temp directory.
pub fn test_conductor_db() -> TestDb<DbKindConductor> {
    test_db(DbKindConductor)
}

/// Create a [`TestDb`] of [DbKindWasm], backed by a temp directory.
pub fn test_wasm_db() -> TestDb<DbKindWasm> {
    test_db(DbKindWasm)
}

fn test_db<Kind: DbKindT>(kind: Kind) -> TestDb<Kind> {
    let tmpdir = tempfile::Builder::new()
        .prefix("holochain-test-environments-")
        .suffix(&nanoid::nanoid!())
        .tempdir()
        .unwrap();
    TestDb {
        db: DbWrite::test(tmpdir.path(), kind).expect("Couldn't create test database"),
        dir: tmpdir.into(),
    }
}

/// Create a [`DbWrite`] of [`DbKindT`] in memory.
pub fn test_in_mem_db<Kind: DbKindT>(kind: Kind) -> DbWrite<Kind> {
    DbWrite::test_in_mem(kind).expect("Couldn't create test database")
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

/// A test database in a temp directory
#[derive(Shrinkwrap)]
pub struct TestDb<Kind: DbKindT> {
    #[shrinkwrap(main_field)]
    /// sqlite database
    db: DbWrite<Kind>,
    /// temp directory for this environment
    dir: TestDir,
}

impl<Kind: DbKindT> TestDb<Kind> {
    /// Accessor
    pub fn to_db(&self) -> DbWrite<Kind> {
        self.db.clone()
    }

    /// Accessor
    pub fn persist(&mut self) {
        self.dir.persist()
    }

    /// Dump db to a location.
    pub fn dump(&self, out: &Path) -> std::io::Result<()> {
        std::fs::create_dir(out).ok();
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let mut out = out.to_owned();
                out.push(format!(
                    "backup.{}",
                    path.extension().unwrap().to_string_lossy()
                ));
                std::fs::copy(path, out)?;
            }
        }
        Ok(())
    }

    /// Dump db into `/tmp/test_dbs`.
    pub async fn dump_tmp(&self) {
        dump_tmp(&self.db).await;
    }

    pub fn dna_hash(&self) -> Option<Arc<DnaHash>> {
        match self.db.kind().kind() {
            DbKind::Cache(hash) | DbKind::Dht(hash) => Some(hash),
            DbKind::Authored(cell_id) => Some(Arc::new(cell_id.dna_hash().clone())),
            _ => None,
        }
    }
}

/// Dump db into `/tmp/test_dbs`.
pub async fn dump_tmp<Kind: DbKindT>(env: &DbWrite<Kind>) {
    let mut tmp = std::env::temp_dir();
    tmp.push("test_dbs");
    std::fs::create_dir(&tmp).ok();
    tmp.push("backup.sqlite");
    println!("dumping db to {}", tmp.display());
    std::fs::write(&tmp, b"").unwrap();
    env.read_async(move |txn| -> DatabaseResult<usize> {
        Ok(txn.execute("VACUUM main into ?", [tmp.to_string_lossy()])?)
    })
    .await
    .unwrap();
}

/// A container for all three non-cell environments
pub struct TestDbs {
    /// A test conductor environment
    conductor: DbWrite<DbKindConductor>,
    /// A test wasm environment
    wasm: DbWrite<DbKindWasm>,
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
                println!("Made temp dir permanent at {:?}", d);
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
        let conductor = DbWrite::test(tempdir.path(), DbKindConductor).unwrap();
        let wasm = DbWrite::test(tempdir.path(), DbKindWasm).unwrap();
        Self {
            conductor,
            wasm,
            dir: TestDir::new(tempdir),
            keystore,
        }
    }

    /// Create all three non-cell environments at once with a test keystore
    pub fn new(tempdir: TempDir) -> Self {
        Self::with_keystore(tempdir, holochain_keystore::test_keystore())
    }

    pub fn conductor(&self) -> DbWrite<DbKindConductor> {
        self.conductor.clone()
    }

    pub fn wasm(&self) -> DbWrite<DbKindWasm> {
        self.wasm.clone()
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

#[cfg(test)]
mod tests {
    use holochain_sqlite::error::DatabaseResult;
    use holochain_sqlite::rusqlite::Transaction;

    fn _dbg_db_schema(db_name: &str, conn: &Transaction) {
        #[allow(dead_code)]
        #[derive(Debug)]
        pub struct Schema {
            pub ty: String,
            pub name: String,
            pub tbl_name: String,
            pub rootpage: u64,
            pub sql: Option<String>,
        }

        let mut statement = conn.prepare("select * from sqlite_schema").unwrap();
        let iter = statement
            .query_map([], |row| {
                Ok(Schema {
                    ty: row.get(0)?,
                    name: row.get(1)?,
                    tbl_name: row.get(2)?,
                    rootpage: row.get(3)?,
                    sql: row.get(4)?,
                })
            })
            .unwrap();

        println!("~~~ {} START ~~~", &db_name);
        for i in iter {
            dbg!(&i);
        }
        println!("~~~ {} END ~~~", &db_name);
    }

    #[tokio::test(flavor = "multi_thread")]
    pub async fn dbg_db_schema() {
        super::test_conductor_db()
            .db
            .read_async(move |txn| -> DatabaseResult<()> {
                _dbg_db_schema("conductor", txn);
                Ok(())
            })
            .await
            .unwrap();
    }
}
