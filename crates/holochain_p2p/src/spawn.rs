use crate::actor::*;
use crate::event::*;

mod actor;
use actor::*;

/// Spawn a new HolochainP2p actor.  Conductor will call this on initialization.
pub async fn spawn_holochain_p2p(
    config: kitsune_p2p::KitsuneP2pConfig,
) -> HolochainP2pResult<(
    ghost_actor::GhostSender<HolochainP2p>,
    HolochainP2pEventReceiver,
)> {
    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);

    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let channel_factory = builder.channel_factory().clone();

    let sender = channel_factory.create_channel::<HolochainP2p>().await?;

    tokio::task::spawn(
        builder.spawn(HolochainP2pActor::new(config, channel_factory, evt_send).await?),
    );

    Ok((sender, evt_recv))
}
