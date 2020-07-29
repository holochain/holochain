//! This is a temporary quick-hack gossip module for use with the
//! in-memory / full-sync / non-sharded networking module

use std::sync::Arc;
use crate::{*, types::actor::KitsuneP2pResult};

ghost_actor::ghost_chan! {
    /// "Event" requests emitted by the gossip module
    pub chan GossipEvent<crate::KitsuneP2pError> {
        /// get a list of agents we know about
        fn list_neighbor_agents(
            space: Arc<KitsuneSpace>,
        ) -> Vec<Arc<KitsuneAgent>>;
    }
}

pub type GossipEventReceiver = futures::channel::mpsc::Receiver<GossipEvent>;

/// spawn a gossip module to control gossip for a space
pub async fn spawn_gossip_module(
    space: Arc<KitsuneSpace>,
) -> KitsuneP2pResult<
    GossipEventReceiver,
> {
    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);

    tokio::task::spawn(gossip_loop(space, evt_send));

    Ok(evt_recv)
}

/// the gossip module is not an actor because we want to pause while
/// awaiting requests - not process requests in parallel.
async fn gossip_loop(
    space: Arc<KitsuneSpace>,
    evt_send: futures::channel::mpsc::Sender<GossipEvent>,
) -> KitsuneP2pResult<()> {
    loop {
        let _agents = evt_send.list_neighbor_agents(
            space.clone(),
        ).await?;

        tokio::time::delay_for(std::time::Duration::from_millis(10)).await;
    }
}
