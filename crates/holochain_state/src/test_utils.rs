//! Helpers for unit tests

use holochain_sqlite::prelude::*;
use holochain_types::prelude::*;
use holochain_zome_types::test_utils::fake_cell_id;
use shrinkwraprs::Shrinkwrap;
use std::sync::Arc;
use tempdir::TempDir;

/// Create a [TestEnv] of [DbKind::Cell], backed by a temp directory.
pub fn test_cell_env() -> TestEnv {
    let cell_id = fake_cell_id(1);
    test_env(DbKind::Cell(cell_id))
}

/// Create a [TestEnv] of [DbKind::Conductor], backed by a temp directory.
pub fn test_conductor_env() -> TestEnv {
    test_env(DbKind::Conductor)
}

/// Create a [TestEnv] of [DbKind::Wasm], backed by a temp directory.
pub fn test_wasm_env() -> TestEnv {
    test_env(DbKind::Wasm)
}

/// Create a [TestEnv] of [DbKind::P2p], backed by a temp directory.
pub fn test_p2p_env() -> TestEnv {
    test_env(DbKind::P2p)
}

fn test_env(kind: DbKind) -> TestEnv {
    let tmpdir = Arc::new(TempDir::new("holochain-test-environments").unwrap());
    TestEnv {
        env: EnvWrite::test(&tmpdir, kind, test_keystore()).expect("Couldn't create test database"),
        tmpdir,
    }
}

/// Create a fresh set of test environments with a new TempDir
pub fn test_environments() -> TestEnvs {
    let tempdir = TempDir::new("holochain-test-environments").unwrap();
    TestEnvs::new(tempdir)
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

/// A test database in a temp directory
#[derive(Clone, Shrinkwrap)]
pub struct TestEnv {
    #[shrinkwrap(main_field)]
    /// sqlite database
    env: EnvWrite,
    /// temp directory for this environment
    tmpdir: Arc<TempDir>,
}

impl TestEnv {
    /// Accessor
    pub fn env(&self) -> EnvWrite {
        self.env.clone()
    }

    /// Accessor
    pub fn tmpdir(&self) -> Arc<TempDir> {
        self.tmpdir.clone()
    }
}

#[derive(Clone)]
/// A container for all three non-cell environments
pub struct TestEnvs {
    /// A test conductor environment
    conductor: EnvWrite,
    /// A test wasm environment
    wasm: EnvWrite,
    /// A test p2p environment
    p2p: EnvWrite,
    /// The shared root temp dir for these environments
    tempdir: Arc<TempDir>,
}

#[allow(missing_docs)]
impl TestEnvs {
    /// Create all three non-cell environments at once
    pub fn new(tempdir: TempDir) -> Self {
        use DbKind::*;
        let keystore = test_keystore();
        let conductor = EnvWrite::test(&tempdir, Conductor, keystore.clone()).unwrap();
        let wasm = EnvWrite::test(&tempdir, Wasm, keystore.clone()).unwrap();
        let p2p = EnvWrite::test(&tempdir, P2p, keystore.clone()).unwrap();
        Self {
            conductor,
            wasm,
            p2p,
            tempdir: Arc::new(tempdir),
        }
    }

    pub fn conductor(&self) -> EnvWrite {
        self.conductor.clone()
    }

    pub fn wasm(&self) -> EnvWrite {
        self.wasm.clone()
    }

    pub fn p2p(&self) -> EnvWrite {
        self.p2p.clone()
    }

    /// Get the root temp dir for these environments
    pub fn tempdir(&self) -> Arc<TempDir> {
        self.tempdir.clone()
    }

    pub fn keystore(&self) -> KeystoreSender {
        self.conductor.keystore()
    }
}

/// Produce file and line number info at compile-time
#[macro_export]
macro_rules! here {
    ($test: expr) => {
        concat!($test, " !!!_LOOK HERE:---> ", file!(), ":", line!())
    };
}
