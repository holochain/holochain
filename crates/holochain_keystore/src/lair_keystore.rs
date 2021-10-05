//! Keystore backed by legacy_lair_client.

use crate::*;
use legacy_lair_api::*;

/// Spawn a new keystore backed by legacy_lair_client.
pub async fn spawn_lair_keystore(
    lair_dir: Option<&std::path::Path>,
    passphrase: sodoken::BufRead,
) -> KeystoreApiResult<KeystoreSender> {
    let mut config = Config::builder();
    if let Some(lair_dir) = lair_dir {
        config = config.set_root_path(lair_dir);
    }
    let config = config.build();

    let api = legacy_lair_client::assert_running_lair_and_connect(config, passphrase).await?;

    Ok(api)
}
