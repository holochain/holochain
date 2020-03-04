

use crate::{env::{Env, create_lmdb_env, EnvArc}};
use rkv::{Rkv};
use std::sync::{Arc, RwLock};
use tempdir::TempDir;

pub fn test_env() -> EnvArc {
    let tmpdir = TempDir::new("skunkworx").unwrap();
    create_lmdb_env(tmpdir.path())
}
