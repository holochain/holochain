//! Helpers for unit tests

use crate::{
    env::{EnvironmentKind, EnvironmentWrite},
    prelude::BufKey,
};
use holochain_types::test_utils::fake_cell_id;
use shrinkwraprs::Shrinkwrap;
use std::sync::Arc;
use tempdir::TempDir;

/// Create an [TestEnvironment] of [EnvironmentKind::Cell], backed by a temp directory
pub fn test_cell_env() -> TestEnvironment {
    let cell_id = fake_cell_id(1);
    test_env(EnvironmentKind::Cell(cell_id))
}

/// Create an [TestEnvironment] of [EnvironmentKind::Conductor], backed by a temp directory
pub fn test_conductor_env() -> TestEnvironment {
    test_env(EnvironmentKind::Conductor)
}

/// Create an [TestEnvironment] of [EnvironmentKind::Wasm], backed by a temp directory
pub fn test_wasm_env() -> TestEnvironment {
    test_env(EnvironmentKind::Wasm)
}

/// Generate a test keystore pre-populated with a couple test keypairs.
pub fn test_keystore() -> holochain_keystore::KeystoreSender {
    use holochain_keystore::KeystoreSenderExt;
    let _ = holochain_crypto::crypto_init_sodium();

    tokio_safe_block_on::tokio_safe_block_on(
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
    .unwrap()
}

fn test_env(kind: EnvironmentKind) -> TestEnvironment {
    let tmpdir = Arc::new(TempDir::new("holochain-test-environments").unwrap());
    TestEnvironment {
        env: EnvironmentWrite::new(tmpdir.path(), kind, test_keystore())
            .expect("Couldn't create test LMDB environment"),
        tmpdir,
    }
}

/// A test lmdb environment with test directory
#[derive(Clone, Shrinkwrap)]
pub struct TestEnvironment {
    #[shrinkwrap(main_field)]
    /// lmdb environment
    pub env: EnvironmentWrite,
    /// temp directory for this environment
    pub tmpdir: Arc<TempDir>,
}

impl TestEnvironment {
    /// Accessor
    pub fn env(&self) -> EnvironmentWrite {
        self.env.clone()
    }

    /// Accessor
    pub fn tmpdir(&self) -> Arc<TempDir> {
        self.tmpdir.clone()
    }
}

// FIXME: Currently the test environments using TempDirs above immediately
// delete the temp dirs after installation. If we ever have cases where we
// want to flush to disk, this will probably fail. In that case we want to
// use something like this, which owns the TempDir so it lives long enough
//
// /// A wrapper around an EnvironmentWrite which includes a reference to a TempDir,
// /// so that when the TestEnvironment goes out of scope, the tempdir is deleted
// /// from the filesystem
// #[derive(Shrinkwrap)]
// pub struct TestEnvironment(#[shrinkwrap(main_field)] EnvironmentWrite, TempDir);

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
