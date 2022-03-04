use futures::FutureExt;

use crate::spawn::MockKitsuneP2pEventHandler;

use super::*;
use crate::fixt::*;
use fixt::prelude::*;

mod bloom;
mod common;
mod test_two_nodes;

impl ShardedGossipLocal {
    /// Create an instance suitable for testing
    pub fn test(
        gossip_type: GossipType,
        evt_sender: EventSender,
        host: HostApi,
        inner: ShardedGossipLocalState,
    ) -> Self {
        // TODO: randomize space
        let space = Arc::new(KitsuneSpace::new([0; 36].to_vec()));
        Self {
            gossip_type,
            tuning_params: Default::default(),
            space,
            evt_sender,
            _host: host,
            inner: Share::new(inner),
            closing: std::sync::atomic::AtomicBool::new(false),
        }
    }
}
