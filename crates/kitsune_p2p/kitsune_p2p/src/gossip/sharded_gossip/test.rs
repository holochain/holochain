use rand::Rng;

use crate::event::KitsuneP2pEventSender;

use super::*;

impl<E: KitsuneP2pEventSender> ShardedGossip<E> {
    pub fn test(gossip_type: GossipType) -> Self {
        // TODO: randomize space
        let space = Arc::new(KitsuneSpace::new([0; 36].to_vec()));
        Self {
            gossip_type,
            tuning_params: Default::default(),
            ep_hnd: todo!(),
            space,
            evt_sender: todo!(),
            inner: todo!(),
        }
    }
}
