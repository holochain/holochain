use crate::actor::*;
use crate::event::*;
use crate::HostApi;
use crate::HostApiLegacy;
use kitsune_p2p_types::config::KitsuneP2pConfig;

mod actor;
pub(crate) use actor::meta_net;
use actor::*;

#[cfg(any(test, feature = "test_utils"))]
pub use actor::MockKitsuneP2pEventHandler;

use self::meta_net::PreflightUserData;

/// Spawn a new KitsuneP2p actor.
pub async fn spawn_kitsune_p2p(
    config: KitsuneP2pConfig,
    tls_config: kitsune_p2p_types::tls::TlsConfig,
    host: HostApi,
    preflight_user_data: PreflightUserData,
) -> KitsuneP2pResult<(
    ghost_actor::GhostSender<KitsuneP2p>,
    KitsuneP2pEventReceiver,
)> {
    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);
    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let channel_factory = builder.channel_factory().clone();

    let internal_sender = channel_factory.create_channel::<Internal>().await?;

    let sender = channel_factory.create_channel::<KitsuneP2p>().await?;
    let host = HostApiLegacy::new(host, evt_send);

    tokio::task::spawn(
        builder.spawn(
            KitsuneP2pActor::new(
                config,
                tls_config,
                channel_factory,
                internal_sender,
                host,
                preflight_user_data,
            )
            .await?,
        ),
    );

    Ok((sender, evt_recv))
}
