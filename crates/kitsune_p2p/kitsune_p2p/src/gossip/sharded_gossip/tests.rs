use super::*;
use crate::spawn::MockKitsuneP2pEventHandler;
use futures::FutureExt;
use rand::Rng;

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
        let mut space = vec![0; 36];
        rand::thread_rng().fill(&mut space[..]);
        let space = KitsuneSpace::new(space);
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
