//! Keystore backed by legacy_lair_client.

use crate::*;
use lair_keystore_api_0_0::*;

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

    let api = lair_keystore_client_0_0::assert_running_lair_and_connect(config, passphrase).await?;

    Ok(MetaLairClient::Legacy(api))
}
