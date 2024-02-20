use std::sync::Arc;

use futures::lock::Mutex;
use kitsune_p2p_types::dependencies::lair_keystore_api::{self, LairClient, LairResult};
use tokio::runtime::Handle;
use tx5::deps::{hc_seed_bundle::PwHashLimits, sodoken};

/// Construct a new TestKeystore with the new lair api.
pub async fn spawn_test_keystore() -> LairResult<Arc<Mutex<LairClient>>> {
    // in-memory secure random passphrase
    let passphrase = sodoken::BufWrite::new_mem_locked(32)?;
    sodoken::random::bytes_buf(passphrase.clone()).await?;

    // in-mem / in-proc config
    let config = Arc::new(
        PwHashLimits::Minimum
            .with_exec(|| {
                lair_keystore_api::config::LairServerConfigInner::new("/", passphrase.to_read())
            })
            .await?,
    );

    // the keystore
    let keystore = lair_keystore_api::in_proc_keystore::InProcKeystore::new(
        config,
        lair_keystore_api::mem_store::create_mem_store_factory(),
        passphrase.to_read(),
    )
    .await?;

    // return the client
    let client = keystore.new_client().await?;
    Ok(Arc::new(Mutex::new(client)))
}

pub fn test_keystore() -> Arc<Mutex<LairClient>> {
    tokio::task::block_in_place(move || {
        Handle::current().block_on(async { spawn_test_keystore().await.unwrap() })
    })
}
