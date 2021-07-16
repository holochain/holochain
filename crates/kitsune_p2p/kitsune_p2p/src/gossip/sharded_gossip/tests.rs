use crate::spawn::MockKitsuneP2pEventHandler;

use super::*;

impl ShardedGossip {
    pub fn test(
        gossip_type: GossipType,
        evt_sender: EventSender,
        inner: ShardedGossipInner,
    ) -> Self {
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
    let evt_handler = MockKitsuneP2pEventHandler::new();
    let evt_sender = todo!("make sender from handler");
    let gossip = ShardedGossip::test(GossipType::Recent, evt_sender, Default::default());

    gossip
        .inner
        .share_mut(|mut state, _| {
            state.incoming.push_back(todo!());
            Ok(())
        })
        .unwrap();
}
