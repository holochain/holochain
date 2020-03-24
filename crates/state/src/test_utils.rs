//! Helpers for unit tests

use crate::env::{create_lmdb_env, Environment, EnvironmentKind};
use shrinkwraprs::Shrinkwrap;
use tempdir::TempDir;

/// Create an [TestEnvironment] of [EnvironmentKind::Cell], backed by a temp directory
/// TODO: return reference to the TempDir so it can be removed
pub fn test_cell_env() -> TestEnvironment {
    test_env(EnvironmentKind::Cell)
}

/// Create an [TestEnvironment] of [EnvironmentKind::Conductor], backed by a temp directory
/// TODO: return reference to the TempDir so it can be removed
pub fn test_conductor_env() -> TestEnvironment {
    test_env(EnvironmentKind::Conductor)
}

fn test_env(kind: EnvironmentKind) -> TestEnvironment {
    let tmpdir = TempDir::new("holochain-test-environments").unwrap();
    let env = create_lmdb_env(tmpdir.path(), kind).expect("Couldn't create test LMDB environment");
    TestEnvironment(env, tmpdir)
}

/// A wrapper around an Environment which includes a reference to a TempDir,
/// so that when the TestEnvironment goes out of scope, the tempdir is deleted
/// from the filesystem
#[derive(Shrinkwrap)]
pub struct TestEnvironment(#[shrinkwrap(main_field)] Environment, TempDir);
