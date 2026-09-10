//! Keystore backed by lair_keystore_api.

use crate::*;
use ::lair_keystore::dependencies::lair_keystore_api;
use lair_keystore_api::in_proc_keystore::InProcKeystore;
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
///
/// This runs the lair server inside this process and talks to it over
/// in-memory channels. It deliberately does *not* go through
/// `lair_keystore::server::StandaloneServer`, which would bind the unix domain
/// socket named by `config.connection_url` and connect back to it over IPC.
///
/// Binding that socket makes holochain unusable on iOS. `sockaddr_un.sun_path`
/// is capped at ~104 bytes, while an iOS app container is ~84 bytes (device) to
/// ~162 bytes (simulator) before anything is appended — so the conventional
/// data directory (`Library/Application Support/<bundle-id>/...`) can never
/// hold the socket, and startup fails with
/// `InvalidInput: "path must be shorter than SUN_LEN"`.
///
/// The standalone server was used here to get its pid-checks; those are kept
/// below by calling `lair_keystore::pid_check::pid_check` directly, so the only behaviour dropped is
/// the socket itself. `config.connection_url` is still honoured for the server
/// identity key (`?k=`), so existing configs keep working unchanged — only the
/// path component goes unused, and no `socket` file is created.
pub async fn spawn_lair_keystore_in_proc(
    config_path: &PathBuf,
    passphrase: SharedLockedArray,
) -> LairResult<MetaLairClient> {
    let config = get_config(config_path, passphrase.clone()).await?;

    // Same guard StandaloneServer::new applies before touching the store, so a
    // second process cannot claim a store this one is using.
    {
        let config = config.clone();
        // TODO - make pid_check async friendly
        tokio::task::spawn_blocking(move || ::lair_keystore::pid_check::pid_check(&config))
            .await
            .map_err(one_err::OneErr::new)??;
    }

    // The same persistent sqlcipher store StandaloneServer would have built.
    let store_factory =
        ::lair_keystore::create_sql_pool_factory(&config.store_file, &config.database_salt);

    let keystore = InProcKeystore::new(config, store_factory, passphrase).await?;
    let client = keystore.new_client().await?;

    // just incase a Drop gets impld at some point...
    std::mem::forget(keystore);

    // In-process transport cannot drop the way an IPC socket can, so the
    // connection-health checker MetaLairClient::new sets up has nothing to do;
    // hand it a sender whose receiver is closed (as spawn_mem_keystore does).
    let (s, _) = tokio::sync::mpsc::unbounded_channel();
    Ok(MetaLairClient(Arc::new(parking_lot::Mutex::new(client)), s))
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
