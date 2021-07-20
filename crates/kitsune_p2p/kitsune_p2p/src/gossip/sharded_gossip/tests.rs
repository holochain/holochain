use futures::FutureExt;
use ghost_actor::{GhostControlHandler, GhostResult};

use crate::spawn::MockKitsuneP2pEventHandler;

use super::*;
use crate::fixt::*;
use fixt::prelude::*;

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

    // TODO: Arbitrary impl for Tx2Cert
    let cert = Tx2Cert(Arc::new((CertDigest::from(vec![0]), "".into(), "".into())));
    let msg = ShardedGossipWire::Initiate(Initiate { intervals: vec![] });
    let outgoing = gossip.process_incoming(cert, msg).await.unwrap();

    assert_eq!(outgoing, vec![]);
    // gossip
    //     .inner
    //     .share_mut(|i, _| Ok(todo!("make assertions about internal state")))
    //     .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn sharded_sanity_test() {
    let alice_agent_info = fixt!(AgentInfoSigned);
    let alice_agent = alice_agent_info.agent.clone();
    let mut evt_handler = MockKitsuneP2pEventHandler::new();
    evt_handler
        .expect_handle_query_agent_info_signed()
        .return_once(move |_| {
            Ok(async move { Ok(vec![alice_agent_info.clone()]) }
                .boxed()
                .into())
        });
    evt_handler
        .expect_handle_query_gossip_agents()
        .returning(|_| {
            Ok(
                async { Ok(vec![(Arc::new(fixt!(KitsuneAgent)), ArcInterval::Full)]) }
                    .boxed()
                    .into(),
            )
        });
    evt_handler
        .expect_handle_hashes_for_time_window()
        .returning(|_| {
            Ok(
                async { Ok(Some((vec![Arc::new(KitsuneOpHash(vec![0]))], 0..u64::MAX))) }
                    .boxed()
                    .into(),
            )
        });
    let (evt_sender, _) = spawn_handler(evt_handler).await;
    let alice = ShardedGossip::test(GossipType::Historical, evt_sender, Default::default());

    let mut evt_handler = MockKitsuneP2pEventHandler::new();
    evt_handler
        .expect_handle_query_agent_info_signed()
        .returning(|_| Ok(async { Ok(vec![fixt!(AgentInfoSigned)]) }.boxed().into()));
    evt_handler
        .expect_handle_get_agent_info_signed()
        .returning(|_| Ok(async { Ok(Some(fixt!(AgentInfoSigned))) }.boxed().into()));
    evt_handler
        .expect_handle_query_gossip_agents()
        .returning(|_| {
            Ok(
                async { Ok(vec![(Arc::new(fixt!(KitsuneAgent)), ArcInterval::Full)]) }
                    .boxed()
                    .into(),
            )
        });
    evt_handler
        .expect_handle_hashes_for_time_window()
        .returning(|_| {
            Ok(
                async { Ok(Some((vec![Arc::new(KitsuneOpHash(vec![0]))], 0..u64::MAX))) }
                    .boxed()
                    .into(),
            )
        });
    let (evt_sender, _) = spawn_handler(evt_handler).await;
    let bob = ShardedGossip::test(GossipType::Historical, evt_sender, Default::default());

    // Set alice initial state
    alice
        .inner
        .share_mut(|i, _| {
            i.local_agents.insert(alice_agent);
            assert_eq!(i.state_map.current_rounds().len(), 0);
            Ok(())
        })
        .unwrap();

    // TODO: Arbitrary impl for Tx2Cert
    let cert = Tx2Cert(Arc::new((CertDigest::from(vec![0]), "".into(), "".into())));

    // Set bob initial state
    bob.inner
        .share_mut(|i, _| {
            i.local_agents.insert(Arc::new(fixt!(KitsuneAgent)));
            assert_eq!(i.state_map.current_rounds().len(), 0);
            Ok(())
        })
        .unwrap();

    let (_, _, bob_outgoing) = bob.try_initiate().await.unwrap().unwrap();
    dbg!(&bob_outgoing);

    let alice_outgoing = alice
        .process_incoming(cert.clone(), bob_outgoing)
        .await
        .unwrap();

    assert_eq!(alice_outgoing.len(), 5);
    alice
        .inner
        .share_mut(|i, _| {
            assert_eq!(i.state_map.current_rounds().len(), 1);
            Ok(())
        })
        .unwrap();

    let mut bob_outgoing = Vec::new();
    for incoming in alice_outgoing {
        dbg!(&incoming);
        let outgoing = bob.process_incoming(cert.clone(), incoming).await.unwrap();
        dbg!(&outgoing);
        bob_outgoing.extend(outgoing);
    }
    assert_eq!(bob_outgoing.len(), 4);
    bob.inner
        .share_mut(|i, _| {
            assert_eq!(i.state_map.current_rounds().len(), 1);
            Ok(())
        })
        .unwrap();
}
