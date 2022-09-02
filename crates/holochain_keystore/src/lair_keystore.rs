//! Keystore backed by lair_keystore_api.

use crate::*;
use kitsune_p2p_types::dependencies::{lair_keystore_api, url2};
use lair_keystore_api::LairResult;

/// Spawn a new keystore backed by lair_keystore_api.
pub async fn spawn_lair_keystore(
    connection_url: url2::Url2,
    passphrase: sodoken::BufRead,
) -> LairResult<MetaLairClient> {
    MetaLairClient::new(connection_url, passphrase).await
}
