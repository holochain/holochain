use futures::future::FutureExt;

use crate::actor::*;
use crate::event::*;

mod actor;
use actor::*;

/// Spawn a new HolochainP2p actor.
pub async fn spawn_holochain_p2p(
) -> HolochainP2pResult<(HolochainP2pSender, HolochainP2pEventReceiver)> {
    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);
    let (sender, driver) = HolochainP2pSender::ghost_actor_spawn(Box::new(|internal_sender| {
        async move { HolochainP2pActor::new(internal_sender, evt_send) }
            .boxed()
            .into()
    }))
    .await?;
    tokio::task::spawn(driver);
    Ok((sender, evt_recv))
}
