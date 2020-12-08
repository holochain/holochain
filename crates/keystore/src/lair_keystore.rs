//! Keystore backed by lair_keystore_client.

use crate::*;
use ghost_actor::dependencies::futures::future::FutureExt;
use ghost_actor::dependencies::futures::stream::StreamExt;
use lair_keystore_api::actor::*;
use lair_keystore_api::*;

/// Spawn a new keystore backed by lair_keystore_client.
pub async fn spawn_lair_keystore(
    lair_dir: Option<&std::path::Path>,
) -> KeystoreApiResult<KeystoreSender> {
    let mut config = Config::builder();
    if let Some(lair_dir) = lair_dir {
        config = config.set_root_path(lair_dir);
    }
    let config = config.build();
    let (api, mut evt) = lair_keystore_client::assert_running_lair_and_connect(config).await?;

    // TODO - actual passphrase handling
    tokio::task::spawn(async move {
        while let Some(r) = evt.next().await {
            match r {
                LairClientEvent::RequestUnlockPassphrase { respond, .. } => {
                    respond.respond(Ok(async move { Ok("[blank-passphrase]".to_string()) }
                        .boxed()
                        .into()));
                }
            }
        }
    });

    Ok(api)
}
