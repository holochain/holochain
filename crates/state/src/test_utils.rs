//! Helpers for unit tests

use crate::env::{create_lmdb_env, Environment};

use tempdir::TempDir;

/// Create an [Environment] backed by a temp directory
/// TODO: return reference to the TempDir so it can be removed
pub fn test_env() -> Environment {
    let tmpdir = TempDir::new("skunkworx").unwrap();
    create_lmdb_env(tmpdir.path()).expect("Couldn't create test LMDB environment")
}
