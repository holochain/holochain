use std::sync::Once;
use sx_state::env::{create_lmdb_env, Environment};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use tempdir::TempDir;

static TRACING: Once = Once::new();

pub fn test_env() -> Environment {
    let tmpdir = TempDir::new("skunkworx").unwrap();
    TRACING.call_once(|| tracing());
    create_lmdb_env(tmpdir.path()).expect("Couldn't create test LMDB environment")
}

fn tracing() {
    if let Some(_) = option_env!("RUST_LOG") {
        let subscriber = FmtSubscriber::builder()
            .with_env_filter(EnvFilter::from_default_env())
            .finish();
        tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing on");
    }
}
