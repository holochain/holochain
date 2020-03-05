

use crate::{env::{create_lmdb_env, EnvArc}};


use tempdir::TempDir;

pub fn test_env() -> EnvArc {
    let tmpdir = TempDir::new("skunkworx").unwrap();
    create_lmdb_env(tmpdir.path()).expect("Couldn't create test LMDB environment")
}
