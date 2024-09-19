use crate::actor::*;
use crate::event::*;
use crate::HostApi;
use crate::HostApiLegacy;
use kitsune_p2p_types::config::KitsuneP2pConfig;

mod actor;

pub(crate) use actor::meta_net;

#[cfg(feature = "test_utils")]
pub use actor::MockKitsuneP2pEventHandler;

use self::actor::Internal;
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

    // Create a `HostApiLegacy` that is configured to talk to the `KitsuneP2pActor` rather than directly to the Kitsune host.
    let self_host_api = {
        let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);
        let self_host_api = HostApiLegacy::new(host.api.clone(), evt_send);
        channel_factory.attach_receiver(evt_recv).await?;

        self_host_api
    };

    // Create the network. Any events it sends will have to wait to be processed until Kitsune has finished initialising
    // but everything that is needed to construct the network is available now.
    let (ep_hnd, ep_evt, bootstrap_net, maybe_peer_url) = create_meta_net(
        &config,
        tls_config,
        internal_sender.clone(),
        self_host_api.clone(),
        preflight_user_data,
    )
    .await?;

    tokio::task::spawn(
        builder.spawn(
            KitsuneP2pActor::new(
                config,
                channel_factory,
                internal_sender,
                host,
                self_host_api,
                ep_hnd,
                ep_evt,
                bootstrap_net,
                maybe_peer_url,
            )
            .await?,
        ),
    );

    Ok((sender, evt_recv))
}
