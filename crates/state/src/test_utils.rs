use crate::env::{create_lmdb_env, Environment};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use tempdir::TempDir;

pub fn test_env() -> Environment {
    let tmpdir = TempDir::new("skunkworx").unwrap();
    tracing();
    create_lmdb_env(tmpdir.path()).expect("Couldn't create test LMDB environment")
}

fn tracing() {
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing on");
}
