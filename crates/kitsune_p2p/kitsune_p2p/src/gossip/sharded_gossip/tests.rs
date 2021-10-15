use arbitrary::Arbitrary;
use futures::FutureExt;

use crate::spawn::MockKitsuneP2pEventHandler;

use super::*;
use crate::fixt::*;
use fixt::prelude::*;

mod common;
mod handler_builder;
mod test_local_sync;
mod test_two_nodes;

impl ShardedGossipLocal {
    pub fn test(
        gossip_type: GossipType,
        evt_sender: EventSender,
        inner: ShardedGossipLocalState,
    ) -> Self {
        // TODO: randomize space
        let space = Arc::new(KitsuneSpace::new([0; 36].to_vec()));
        Self {
            gossip_type,
            tuning_params: Default::default(),
            space,
            evt_sender,
            inner: Share::new(inner),
            closing: std::sync::atomic::AtomicBool::new(false),
        }
    }
}
