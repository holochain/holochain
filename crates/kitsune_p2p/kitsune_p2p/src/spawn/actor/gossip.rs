//! This is a temporary quick-hack gossip module for use with the
//! in-memory / full-sync / non-sharded networking module

use crate::{types::actor::KitsuneP2pResult, *};
use std::sync::Arc;

ghost_actor::ghost_chan! {
    /// "Event" requests emitted by the gossip module
    pub chan GossipEvent<crate::KitsuneP2pError> {
        /// get a list of agents we know about
        fn list_neighbor_agents() -> Vec<Arc<KitsuneAgent>>;
    }
}

pub type GossipEventReceiver = futures::channel::mpsc::Receiver<GossipEvent>;

/// spawn a gossip module to control gossip for a space
pub fn spawn_gossip_module() -> GossipEventReceiver {
    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);

    tokio::task::spawn(gossip_loop(evt_send));

    evt_recv
}

/// the gossip module is not an actor because we want to pause while
/// awaiting requests - not process requests in parallel.
async fn gossip_loop(
    evt_send: futures::channel::mpsc::Sender<GossipEvent>,
) -> KitsuneP2pResult<()> {
    let mut gossip_data = GossipData::new(evt_send);
    loop {
        gossip_data.take_action().await?;

        tokio::time::delay_for(std::time::Duration::from_millis(10)).await;
    }
}

struct GossipData {
    evt_send: futures::channel::mpsc::Sender<GossipEvent>,
    pending_gossip_list: Vec<Arc<KitsuneAgent>>,
}

impl GossipData {
    pub fn new(evt_send: futures::channel::mpsc::Sender<GossipEvent>) -> Self {
        Self {
            evt_send,
            pending_gossip_list: Vec::with_capacity(0),
        }
    }

    pub async fn take_action(&mut self) -> KitsuneP2pResult<()> {
        if self.pending_gossip_list.is_empty() {
            self.fetch_pending_gossip_list().await?;
        } else {
            self.process_next_gossip().await?;
        }
        Ok(())
    }

    async fn fetch_pending_gossip_list(&mut self) -> KitsuneP2pResult<()> {
        self.pending_gossip_list = self.evt_send.list_neighbor_agents().await?;
        // sort?? randomize??
        Ok(())
    }

    async fn process_next_gossip(&mut self) -> KitsuneP2pResult<()> {
        // todo SOMETHING!!
        Ok(())
    }
}
