//! Helpers for unit tests

use crate::env::{EnvironmentKind, EnvironmentWrite};
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
    use holochain_keystore::KeystoreApiSender;
    use std::convert::TryFrom;
    let _ = holochain_crypto::crypto_init_sodium();

    tokio_safe_block_on::tokio_safe_block_on(
        async move {
            let keystore = holochain_keystore::test_keystore::spawn_test_keystore(vec![
                holochain_keystore::test_keystore::MockKeypair {
                    pub_key: holo_hash::AgentPubKey::try_from(
                        "uhCAkw-zrttiYpdfAYX4fR6W8DPUdheZJ-1QsRA4cTImmzTYUcOr4",
                    )
                    .unwrap(),
                    sec_key: vec![
                        220, 218, 15, 212, 178, 51, 204, 96, 121, 97, 6, 205, 179, 84, 80, 159, 84,
                        163, 193, 46, 127, 15, 47, 91, 134, 106, 72, 72, 51, 76, 26, 16, 195, 236,
                        235, 182, 216, 152, 165, 215, 192, 97, 126, 31, 71, 165, 188, 12, 245, 29,
                        133, 230, 73, 251, 84, 44, 68, 14, 28, 76, 137, 166, 205, 54,
                    ],
                },
                holochain_keystore::test_keystore::MockKeypair {
                    pub_key: holo_hash::AgentPubKey::try_from(
                        "uhCAkomHzekU0-x7p62WmrusdxD2w9wcjdajC88688JGSTEo6cbEK",
                    )
                    .unwrap(),
                    sec_key: vec![
                        170, 205, 134, 46, 233, 225, 100, 162, 101, 124, 207, 157, 12, 131, 239,
                        244, 216, 190, 244, 161, 209, 56, 159, 135, 240, 134, 88, 28, 48, 75, 227,
                        244, 162, 97, 243, 122, 69, 52, 251, 30, 233, 235, 101, 166, 174, 235, 29,
                        196, 61, 176, 247, 7, 35, 117, 168, 194, 243, 206, 188, 240, 145, 146, 76,
                        74,
                    ],
                },
            ])
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
#[derive(Shrinkwrap)]
pub struct TestEnvironment {
    #[shrinkwrap(main_field)]
    /// lmdb environment
    pub env: EnvironmentWrite,
    /// temp directory for this environment
    pub tmpdir: Arc<TempDir>,
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
#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DbString(String);

impl AsRef<[u8]> for DbString {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl From<DbString> for Vec<u8> {
    fn from(d: DbString) -> Vec<u8> {
        d.as_ref().to_vec()
    }
}

impl From<Vec<u8>> for DbString {
    fn from(bytes: Vec<u8>) -> Self {
        Self(String::from_utf8(bytes).unwrap())
    }
}

impl From<&str> for DbString {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}
