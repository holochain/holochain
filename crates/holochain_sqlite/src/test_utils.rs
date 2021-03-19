//! Helpers for unit tests

use crate::db::DbKind;
use crate::db::DbWrite;
use crate::prelude::BufKey;
use holochain_keystore::KeystoreSender;
use holochain_zome_types::test_utils::fake_cell_id;
use shrinkwraprs::Shrinkwrap;
use std::sync::Arc;
use tempdir::TempDir;

/// Create a [TestDb] of [DbKind::Cell], backed by a temp directory.
pub fn test_cell_env() -> TestDb {
    let cell_id = fake_cell_id(1);
    test_env(DbKind::Cell(cell_id))
}

/// Create a [TestDb] of [DbKind::Conductor], backed by a temp directory.
pub fn test_conductor_env() -> TestDb {
    test_env(DbKind::Conductor)
}

/// Create a [TestDb] of [DbKind::Wasm], backed by a temp directory.
pub fn test_wasm_env() -> TestDb {
    test_env(DbKind::Wasm)
}

/// Create a [TestDb] of [DbKind::P2p], backed by a temp directory.
pub fn test_p2p_env() -> TestDb {
    test_env(DbKind::P2p)
}

/// Generate a test keystore pre-populated with a couple test keypairs.
pub fn test_keystore() -> holochain_keystore::KeystoreSender {
    use holochain_keystore::KeystoreSenderExt;

    tokio_helper::block_on(
        async move {
            let keystore = holochain_keystore::test_keystore::spawn_test_keystore()
                .await
                .unwrap();

            // pre-populate with our two fixture agent keypairs
            keystore
                .generate_sign_keypair_from_pure_entropy()
                .await
                .unwrap();
            keystore
                .generate_sign_keypair_from_pure_entropy()
                .await
                .unwrap();

            keystore
        },
        std::time::Duration::from_secs(1),
    )
    .expect("timeout elapsed")
}

fn test_env(kind: DbKind) -> TestDb {
    let tmpdir = Arc::new(TempDir::new("holochain-test-environments").unwrap());
    TestDb {
        env: DbWrite::new(tmpdir.path(), kind, test_keystore())
            .expect("Couldn't create test database"),
        tmpdir,
    }
}

/// Create a fresh set of test environments with a new TempDir
pub fn test_environments() -> TestDbs {
    let tempdir = TempDir::new("holochain-test-environments").unwrap();
    TestDbs::new(tempdir)
}

/// A test database in a temp directory
#[derive(Clone, Shrinkwrap)]
pub struct TestDb {
    #[shrinkwrap(main_field)]
    /// sqlite database
    env: DbWrite,
    /// temp directory for this environment
    tmpdir: Arc<TempDir>,
}

impl TestDb {
    /// Accessor
    pub fn env(&self) -> DbWrite {
        self.env.clone()
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
    /// A keystore sender shared by all environments
    keystore: KeystoreSender,
}

#[allow(missing_docs)]
impl TestDbs {
    /// Create all three non-cell environments at once
    pub fn new(tempdir: TempDir) -> Self {
        use DbKind::*;
        let keystore = test_keystore();
        let conductor = DbWrite::new(&tempdir.path(), Conductor, keystore.clone()).unwrap();
        let wasm = DbWrite::new(&tempdir.path(), Wasm, keystore.clone()).unwrap();
        let p2p = DbWrite::new(&tempdir.path(), P2p, keystore.clone()).unwrap();
        Self {
            conductor,
            wasm,
            p2p,
            tempdir: Arc::new(tempdir),
            keystore,
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

    pub fn keystore(&self) -> KeystoreSender {
        self.keystore.clone()
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
