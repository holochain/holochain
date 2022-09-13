//! Keystore backed by lair_keystore_api.

use crate::*;
use ::lair_keystore::server::StandaloneServer;
use kitsune_p2p_types::dependencies::{lair_keystore_api, url2};
use lair_keystore_api::prelude::*;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

/// Spawn a new keystore backed by lair_keystore_api.
pub async fn spawn_lair_keystore(
    connection_url: url2::Url2,
    passphrase: sodoken::BufRead,
) -> LairResult<MetaLairClient> {
    MetaLairClient::new(connection_url, passphrase).await
}

/// Spawn an in-process keystore backed by lair_keystore.
pub async fn spawn_lair_keystore_in_proc(
    config_path: std::path::PathBuf,
    passphrase: sodoken::BufRead,
) -> LairResult<MetaLairClient> {
    let config = get_config(&config_path, passphrase.clone()).await?;
    let connection_url = config.connection_url.clone();

    // rather than using the in-proc server directly,
    // use the actual standalone server so we get the pid-checks, etc
    let mut server = StandaloneServer::new(config).await?;

    server.run(passphrase.clone()).await?;

    // just incase a Drop gets impld at some point...
    std::mem::forget(server);

    // now, just connect to it : )
    spawn_lair_keystore(connection_url.into(), passphrase).await
}

async fn get_config(
    config_path: &std::path::Path,
    passphrase: sodoken::BufRead,
) -> LairResult<LairServerConfig> {
    match read_config(config_path).await {
        Ok(config) => Ok(config),
        Err(_) => write_config(config_path, passphrase).await,
    }
}

async fn read_config(config_path: &std::path::Path) -> LairResult<LairServerConfig> {
    let bytes = tokio::fs::read(config_path).await?;

    let config = LairServerConfigInner::from_bytes(&bytes)?;

    Ok(Arc::new(config))
}

async fn write_config(
    config_path: &std::path::Path,
    passphrase: sodoken::BufRead,
) -> LairResult<LairServerConfig> {
    let lair_root = config_path
        .parent()
        .ok_or_else(|| one_err::OneErr::from("InvalidLairConfigDir"))?;

    tokio::fs::DirBuilder::new()
        .recursive(true)
        .create(&lair_root)
        .await?;

    let config = LairServerConfigInner::new(lair_root, passphrase).await?;

    let mut config_f = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(config_path)
        .await?;

    config_f.write_all(config.to_string().as_bytes()).await?;
    config_f.shutdown().await?;
    drop(config_f);

    Ok(Arc::new(config))
}
