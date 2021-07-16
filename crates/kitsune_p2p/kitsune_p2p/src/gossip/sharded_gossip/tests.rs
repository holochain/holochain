use crate::event::KitsuneP2pEventSender;

use super::*;

impl<E: KitsuneP2pEventSender> ShardedGossip<E> {
    pub fn test(gossip_type: GossipType, evt_sender: E, inner: ShardedGossipInner) -> Self {
        // TODO: randomize space
        let space = Arc::new(KitsuneSpace::new([0; 36].to_vec()));
        Self {
            gossip_type,
            tuning_params: Default::default(),
            space,
            evt_sender,
            inner: Share::new(inner),
        }
    }
}

#[test]
fn test_initiate_accept() {
    let evt_sender = MockKitsuneP2pEventSender::new();
    let gossip = ShardedGossip::test(GossipType::Recent, evt_sender, Default::default());

    gossip
        .inner
        .share_mut(|mut state, _| {
            state.incoming.push_back(todo!());
            Ok(())
        })
        .unwrap();
}
