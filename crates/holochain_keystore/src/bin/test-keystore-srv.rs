use kitsune_p2p_types::dependencies::lair_keystore_api;
use lair_keystore_api::dependencies::hc_seed_bundle;
use lair_keystore_api::ipc_keystore::*;
use lair_keystore_api::prelude::*;
use std::sync::Arc;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let mut arg_iter = std::env::args_os();
    arg_iter.next().unwrap();
    let path = std::path::PathBuf::from(arg_iter.next().expect("require lair path"));

    // set up a passphrase
    let passphrase = sodoken::BufRead::from(&b"passphrase"[..]);

    // create the config for the test server
    let config = Arc::new(
        hc_seed_bundle::PwHashLimits::Minimum
            .with_exec(|| LairServerConfigInner::new(&path, passphrase.clone()))
            .await
            .unwrap(),
    );

    // create an in-process keystore with an in-memory store
    let keystore = IpcKeystoreServer::new(
        config,
        lair_keystore_api::mem_store::create_mem_store_factory(),
        passphrase.clone(),
    )
    .await
    .unwrap();

    let config = keystore.get_config();
    println!("{}", config);

    println!("OK");

    futures::future::pending::<()>().await;
}
