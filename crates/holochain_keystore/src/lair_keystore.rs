//! Keystore backed by legacy_lair_client.

use crate::*;
use kitsune_p2p_types::dependencies::{new_lair_api, url2};
use legacy_lair_api::*;
use new_lair_api::LairResult;

/// Spawn a new keystore backed by legacy_lair_client.
pub async fn spawn_lair_keystore(
    lair_dir: Option<&std::path::Path>,
    passphrase: sodoken::BufRead,
) -> KeystoreApiResult<MetaLairClient> {
    let mut config = Config::builder();
    if let Some(lair_dir) = lair_dir {
        config = config.set_root_path(lair_dir);
    }
    let config = config.build();

    let api = legacy_lair_client::assert_running_lair_and_connect(config, passphrase).await?;

    Ok(MetaLairClient::Legacy(api))
}

/// Spawn a new keystore backed by new_lair_api.
pub async fn spawn_new_lair_keystore(
    connection_url: url2::Url2,
    passphrase: sodoken::BufRead,
) -> LairResult<MetaLairClient> {
    use new_lair_api::ipc_keystore::*;
    let opts = IpcKeystoreClientOptions {
        connection_url: connection_url.into(),
        passphrase,
        exact_client_server_version_match: true,
    };
    let client = ipc_keystore_connect_options(opts).await?;
    Ok(MetaLairClient::NewLair(client))
}
