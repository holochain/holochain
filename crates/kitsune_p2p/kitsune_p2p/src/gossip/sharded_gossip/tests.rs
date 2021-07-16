use ghost_actor::{GhostControlHandler, GhostHandler, GhostResult};

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

async fn spawn_handler<H: KitsuneP2pEventHandler + GhostControlHandler>(
    h: H,
) -> (EventSender, tokio::task::JoinHandle<GhostResult<()>>) {
    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();
    let (tx, rx) = futures::channel::mpsc::channel(4096);
    builder.channel_factory().attach_receiver(rx).await.unwrap();
    let driver = builder.spawn(h);
    (tx, tokio::task::spawn(driver))
}

#[tokio::test(flavor = "multi_thread")]
async fn test_initiate_accept() {
    let evt_handler = MockKitsuneP2pEventHandler::new();
    let (evt_sender, _) = spawn_handler(evt_handler).await;
    let gossip = ShardedGossip::test(GossipType::Recent, evt_sender, Default::default());

    gossip
        .inner
        .share_mut(|mut state, _| {
            state.incoming.push_back(todo!());
            Ok(())
        })
        .unwrap();
}
