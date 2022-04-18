use futures::FutureExt;

use crate::{spawn::MockKitsuneP2pEventHandler, NOISE};

use super::*;
use crate::fixt::*;
use arbitrary::Arbitrary;
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
        let mut u = arbitrary::Unstructured::new(&NOISE);
        let space = KitsuneSpace::arbitrary(&mut u).unwrap();
        let space = Arc::new(space);
        Self {
            gossip_type,
            tuning_params: Default::default(),
            space,
            evt_sender,
            host_api: host,
            inner: Share::new(inner),
            closing: std::sync::atomic::AtomicBool::new(false),
        }
    }
}
