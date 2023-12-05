use futures::FutureExt;

use crate::{spawn::MockKitsuneP2pEventHandler, NOISE};

use super::*;

mod bloom;
mod common;
mod ops;
mod test_two_nodes;

impl ShardedGossipLocal {
    /// Create an instance suitable for testing
    pub fn test(
        gossip_type: GossipType,
        host: HostApiLegacy,
        inner: ShardedGossipLocalState,
    ) -> Self {
        use arbitrary::Arbitrary;
        let mut u = arbitrary::Unstructured::new(&NOISE);
        let space = KitsuneSpace::arbitrary(&mut u).unwrap();
        let space = Arc::new(space);
        let fetch_pool = FetchPool::new_bitwise_or();

        Self {
            gossip_type,
            tuning_params: Default::default(),
            space,
            host_api: host,
            inner: Share::new(inner),
            closing: std::sync::atomic::AtomicBool::new(false),
            fetch_pool,
        }
    }
}
