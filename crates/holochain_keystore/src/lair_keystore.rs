//! Keystore backed by lair_keystore_api.

use crate::*;
use ::lair_keystore::dependencies::lair_keystore_api;
use ::lair_keystore::InProcKeystore;
use lair_keystore_api::prelude::*;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

/// Spawn a new keystore backed by lair_keystore_api.
pub async fn spawn_lair_keystore(
    connection_url: url2::Url2,
    passphrase: SharedLockedArray,
) -> LairResult<MetaLairClient> {
    MetaLairClient::new(connection_url, passphrase).await
}

/// Spawn an in-process keystore backed by lair_keystore.
/// @param config_path - path to the lair config yaml file
pub async fn spawn_lair_keystore_in_proc(
    config_path: &PathBuf,
    passphrase: SharedLockedArray,
) -> LairResult<MetaLairClient> {
    let config = get_config(config_path, passphrase.clone()).await?;

    // use the persistent in-proc keystore rather than a standalone server:
    // no unix domain socket is bound, so this works on platforms whose app
    // data paths exceed `sun_path` (e.g. iOS)
    let keystore = InProcKeystore::new(config, passphrase.clone()).await?;
    let keystore_client = keystore.new_client().await?;

    MetaLairClient::from_client(keystore_client).await
}

async fn get_config(
    config_path: &PathBuf,
    passphrase: SharedLockedArray,
) -> LairResult<LairServerConfig> {
    match read_config(config_path).await {
        Ok(config) => Ok(config),
        Err(_) => write_config(config_path, passphrase).await,
    }
}

async fn read_config(config_path: &PathBuf) -> LairResult<LairServerConfig> {
    let bytes = tokio::fs::read(config_path).await?;

    let config = LairServerConfigInner::from_bytes(&bytes)?;

    Ok(Arc::new(config))
}

async fn write_config(
    config_path: &std::path::Path,
    passphrase: SharedLockedArray,
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
