//! Helpers for unit tests

use crate::env::{EnvironmentKind, EnvironmentRw};
use holochain_types::test_utils::fake_cell_id;
use tempdir::TempDir;

/// Create an [TestEnvironment] of [EnvironmentKind::Cell], backed by a temp directory
pub async fn test_cell_env() -> TestEnvironment {
    let cell_id = fake_cell_id(&nanoid::nanoid!());
    test_env(EnvironmentKind::Cell(cell_id)).await
}

/// Create an [TestEnvironment] of [EnvironmentKind::Conductor], backed by a temp directory
pub async fn test_conductor_env() -> TestEnvironment {
    test_env(EnvironmentKind::Conductor).await
}

/// Create an [TestEnvironment] of [EnvironmentKind::Wasm], backed by a temp directory
pub async fn test_wasm_env() -> TestEnvironment {
    test_env(EnvironmentKind::Wasm).await
}

/// Generate a test keystore pre-populated with a couple test keypairs.
pub fn test_keystore() -> holochain_keystore::KeystoreSender {
    use std::convert::TryFrom;
    let _ = holochain_crypto::crypto_init_sodium();

    tokio_safe_block_on::tokio_safe_block_on(
        async move {
            let mut keystore = holochain_keystore::test_keystore::spawn_test_keystore(vec![
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

async fn test_env(kind: EnvironmentKind) -> TestEnvironment {
    let tmpdir = TempDir::new("holochain-test-environments").unwrap();
    // TODO: Wrap EnvironmentRw along with the TempDir so that it lives longer
    EnvironmentRw::new(tmpdir.path(), kind, test_keystore())
        .await
        .expect("Couldn't create test LMDB environment")
}

type TestEnvironment = EnvironmentRw;

// FIXME: Currently the test environments using TempDirs above immediately
// delete the temp dirs after installation. If we ever have cases where we
// want to flush to disk, this will probably fail. In that case we want to
// use something like this, which owns the TempDir so it lives long enough
//
// /// A wrapper around an EnvironmentRw which includes a reference to a TempDir,
// /// so that when the TestEnvironment goes out of scope, the tempdir is deleted
// /// from the filesystem
// #[derive(Shrinkwrap)]
// pub struct TestEnvironment(#[shrinkwrap(main_field)] EnvironmentRw, TempDir);
