use futures::future::FutureExt;

use crate::actor::*;
use crate::event::*;

mod actor;
use actor::*;

/// Spawn a new KitsuneP2p actor.
pub async fn spawn_kitsune_p2p() -> KitsuneP2pResult<(KitsuneP2pSender, KitsuneP2pEventReceiver)> {
    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);
    let (sender, driver) = KitsuneP2pSender::ghost_actor_spawn(Box::new(|internal_sender| {
        async move { KitsuneP2pActor::new(internal_sender, evt_send) }
            .boxed()
            .into()
    }))
    .await?;
    tokio::task::spawn(driver);
    Ok((sender, evt_recv))
}
