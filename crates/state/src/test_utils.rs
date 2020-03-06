use crate::env::{create_lmdb_env, Environment};

use tempdir::TempDir;

pub fn test_env() -> Environment {
    let tmpdir = TempDir::new("skunkworx").unwrap();
    create_lmdb_env(tmpdir.path()).expect("Couldn't create test LMDB environment")
}
