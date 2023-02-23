use crate::actor::*;
use crate::event::*;
use crate::HostApi;

mod actor;
pub(crate) use actor::meta_net;
use actor::*;

#[cfg(any(test, feature = "test_utils"))]
pub use actor::MockKitsuneP2pEventHandler;
use futures::future::BoxFuture;
use ghost_actor::GhostSender;

/// Spawn a new KitsuneP2p actor.
pub async fn spawn_kitsune_p2p(
    config: crate::KitsuneP2pConfig,
    tls_config: kitsune_p2p_types::tls::TlsConfig,
    host: HostApi,
) -> KitsuneP2pResult<(
    ghost_actor::GhostSender<KitsuneP2p>,
    KitsuneP2pEventReceiver,
)> {
    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);
    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let channel_factory = builder.channel_factory().clone();

    let internal_sender = channel_factory.create_channel::<Internal>().await?;

    let sender = channel_factory.create_channel::<KitsuneP2p>().await?;

    tokio::task::spawn(
        builder.spawn(
            KitsuneP2pActor::new(
                config,
                tls_config,
                channel_factory,
                internal_sender,
                evt_send,
                host,
            )
            .await?,
        ),
    );

    Ok((sender, evt_recv))
}

/// Spawn a new KitsuneP2p actor, using a closure to generate the HostApi.
/// Used for some test cases where the HostApi requires some of the intermediate
/// values created by this function.
pub async fn spawn_kitsune_p2p_with_fn<F, T>(
    config: crate::KitsuneP2pConfig,
    tls_config: kitsune_p2p_types::tls::TlsConfig,
    build_host: F,
) -> KitsuneP2pResult<(
    ghost_actor::GhostSender<KitsuneP2p>,
    KitsuneP2pEventReceiver,
    T,
)>
where
    F: FnOnce(GhostSender<KitsuneP2p>) -> BoxFuture<'static, (T, HostApi)>,
{
    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);
    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let channel_factory = builder.channel_factory().clone();

    let internal_sender = channel_factory.create_channel::<Internal>().await?;

    let sender = channel_factory.create_channel::<KitsuneP2p>().await?;

    let (t, host) = build_host(sender.clone()).await;

    tokio::task::spawn(
        builder.spawn(
            KitsuneP2pActor::new(
                config,
                tls_config,
                channel_factory,
                internal_sender,
                evt_send,
                host,
            )
            .await?,
        ),
    );

    Ok((sender, evt_recv, t))
}
