use arbitrary::Arbitrary;
use futures::FutureExt;
use ghost_actor::{GhostControlHandler, GhostResult};

use crate::{
    gossip::sharded_gossip::initiate::encode_timed_bloom_filter, spawn::MockKitsuneP2pEventHandler,
    NOISE,
};

use super::*;
use crate::fixt::*;
use fixt::prelude::*;

mod test_local_sync;

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
        }
    }
}

/// Create a handler task and produce a Sender for interacting with it
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
    // let mut u = arbitrary::Unstructured::new(&NOISE);
    // let evt_handler = MockKitsuneP2pEventHandler::new();
    // let (evt_sender, _) = spawn_handler(evt_handler).await;
    // let gossip = ShardedGossipLocal::test(GossipType::Recent, evt_sender, Default::default());

    // let cert = Tx2Cert::arbitrary(&mut u).unwrap();
    // let msg = ShardedGossipWire::Initiate(Initiate { intervals: vec![] });
    // let outgoing = gossip.process_incoming(cert, msg).await.unwrap();

    // assert_eq!(outgoing, vec![]);
    // gossip
    //     .inner
    //     .share_mut(|i, _| Ok(todo!("make assertions about internal state")))
    //     .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn sharded_sanity_test() {
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let bob_cert = Tx2Cert::arbitrary(&mut u).unwrap();

    let mut agents = agents().into_iter();
    let alice_agent = agents.next().unwrap();
    let bob_agent = agents.next().unwrap();

    let alice = setup_standard_player(ShardedGossipLocalState {
        local_agents: maplit::hashset! { alice_agent.clone() },
        ..Default::default()
    })
    .await;

    let bob = setup_standard_player(ShardedGossipLocalState {
        local_agents: maplit::hashset! { bob_agent.clone() },
        ..Default::default()
    })
    .await;

    let (_, _, bob_outgoing) = bob.try_initiate().await.unwrap().unwrap();
    let alices_cert = bob
        .inner
        .share_ref(|i| Ok(i.initiate_tgt.as_ref().unwrap().cert().clone()))
        .unwrap();

    let alice_outgoing = alice
        .process_incoming(bob_cert.clone(), bob_outgoing)
        .await
        .unwrap();

    assert_eq!(alice_outgoing.len(), 5);
    alice
        .inner
        .share_mut(|i, _| {
            assert_eq!(i.round_map.current_rounds().len(), 1);
            Ok(())
        })
        .unwrap();

    let mut bob_outgoing = Vec::new();
    dbg!("SENDING TO BOB");
    for incoming in alice_outgoing {
        let outgoing = bob
            .process_incoming(alices_cert.clone(), incoming)
            .await
            .unwrap();
        bob_outgoing.extend(outgoing);
    }
    assert_eq!(bob_outgoing.len(), 8);
    bob.inner
        .share_mut(|i, _| {
            assert_eq!(i.round_map.current_rounds().len(), 1);
            Ok(())
        })
        .unwrap();

    let mut alice_outgoing = Vec::new();
    dbg!("SENDING TO ALICE");
    for incoming in bob_outgoing {
        let outgoing = alice
            .process_incoming(bob_cert.clone(), incoming)
            .await
            .unwrap();
        alice_outgoing.extend(outgoing);
    }
    assert_eq!(alice_outgoing.len(), 4);
    alice
        .inner
        .share_mut(|i, _| {
            assert!(i.initiate_tgt.is_none());
            assert_eq!(i.round_map.current_rounds().len(), 0);
            Ok(())
        })
        .unwrap();

    dbg!("SENDING TO BOB");
    let mut bob_outgoing = Vec::new();
    for incoming in alice_outgoing {
        let outgoing = bob
            .process_incoming(alices_cert.clone(), incoming)
            .await
            .unwrap();
        bob_outgoing.extend(outgoing);
    }
    assert_eq!(bob_outgoing.len(), 0);
    bob.inner
        .share_mut(|i, _| {
            assert!(i.initiate_tgt.is_none());
            assert_eq!(i.round_map.current_rounds().len(), 0);
            Ok(())
        })
        .unwrap();
}

async fn standard_responses(
    agents: Vec<Arc<KitsuneAgent>>,
    with_data: bool,
) -> MockKitsuneP2pEventHandler {
    let mut evt_handler = MockKitsuneP2pEventHandler::new();
    evt_handler
        .expect_handle_query_agent_info_signed()
        .returning({
            let agents = agents.clone();
            move |_| {
                let agents = agents.clone();
                Ok(async move {
                    let mut infos = Vec::new();
                    for agent in agents {
                        infos.push(agent_info(agent).await);
                    }
                    Ok(infos)
                }
                .boxed()
                .into())
            }
        });
    evt_handler
        .expect_handle_get_agent_info_signed()
        .returning({
            let agents = agents.clone();
            move |input| {
                let agents = agents.clone();
                let agent = agents.iter().find(|a| **a == input.agent).unwrap().clone();
                Ok(async move { Ok(Some(agent_info(agent).await)) }
                    .boxed()
                    .into())
            }
        });
    evt_handler.expect_handle_query_gossip_agents().returning({
        move |_| {
            let agents = agents.clone();
            Ok(async move {
                let agents = agents.clone();
                let mut infos = Vec::new();
                for agent in agents {
                    infos.push((agent.clone(), ArcInterval::Full));
                }
                Ok(infos)
            }
            .boxed()
            .into())
        }
    });
    if with_data {
        evt_handler
            .expect_handle_hashes_for_time_window()
            .returning(|_| {
                Ok(async {
                    Ok(Some((
                        vec![Arc::new(KitsuneOpHash(vec![0; 36]))],
                        0..u64::MAX,
                    )))
                }
                .boxed()
                .into())
            });
        evt_handler
            .expect_handle_fetch_op_hashes_for_constraints()
            .returning(|_| {
                Ok(async { Ok(vec![Arc::new(KitsuneOpHash(vec![0; 36]))]) }
                    .boxed()
                    .into())
            });
        evt_handler
            .expect_handle_fetch_op_hash_data()
            .returning(|_| {
                Ok(
                    async { Ok(vec![(Arc::new(KitsuneOpHash(vec![0; 36])), vec![0])]) }
                        .boxed()
                        .into(),
                )
            });
    } else {
        evt_handler
            .expect_handle_hashes_for_time_window()
            .returning(|_| Ok(async { Ok(None) }.boxed().into()));
        evt_handler
            .expect_handle_fetch_op_hashes_for_constraints()
            .returning(|_| Ok(async { Ok(vec![]) }.boxed().into()));
        evt_handler
            .expect_handle_fetch_op_hash_data()
            .returning(|_| Ok(async { Ok(vec![]) }.boxed().into()));
    }
    evt_handler
        .expect_handle_gossip()
        .returning(|_, _, _, _, _| Ok(async { Ok(()) }.boxed().into()));
    evt_handler
}

async fn setup_player(
    state: ShardedGossipLocalState,
    num_agents: usize,
    with_data: bool,
) -> ShardedGossipLocal {
    let agents = std::iter::repeat_with(|| Arc::new(fixt!(KitsuneAgent)))
        .take(num_agents)
        .collect();
    let evt_handler = standard_responses(agents, with_data).await;
    let (evt_sender, _) = spawn_handler(evt_handler).await;
    ShardedGossipLocal::test(GossipType::Historical, evt_sender, state)
}

async fn setup_standard_player(state: ShardedGossipLocalState) -> ShardedGossipLocal {
    setup_player(state, 2, true).await
}

async fn setup_empty_player(state: ShardedGossipLocalState) -> ShardedGossipLocal {
    let evt_handler = standard_responses(agents(), false).await;
    let (evt_sender, _) = spawn_handler(evt_handler).await;
    ShardedGossipLocal::test(GossipType::Historical, evt_sender, state)
}

// maackle: why does this need to be static, can't we just create new agents each time?
fn agents() -> Vec<Arc<KitsuneAgent>> {
    static AGENTS: once_cell::sync::Lazy<Vec<Arc<KitsuneAgent>>> =
        once_cell::sync::Lazy::new(|| {
            vec![Arc::new(fixt!(KitsuneAgent)), Arc::new(fixt!(KitsuneAgent))]
        });
    AGENTS.clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn partial_missing_doesnt_finish() {
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let cert = Tx2Cert::arbitrary(&mut u).unwrap();

    let bob = setup_standard_player(ShardedGossipLocalState {
        round_map: maplit::hashmap! {
            cert.clone() => RoundState {
                common_arc_set: Arc::new(ArcInterval::Full.into()),
                num_ops_blooms: 1,
                increment_ops_complete: true,
                created_at: std::time::Instant::now(),
                round_timeout: u32::MAX,
            }
        }
        .into(),
        ..Default::default()
    })
    .await;

    let incoming = ShardedGossipWire::MissingOps(MissingOps {
        ops: vec![],
        finished: false,
    });

    let outgoing = bob.process_incoming(cert.clone(), incoming).await.unwrap();
    assert_eq!(outgoing.len(), 0);

    bob.inner
        .share_mut(|i, _| {
            assert!(i.initiate_tgt.is_none());
            assert_eq!(i.round_map.current_rounds().len(), 1);
            Ok(())
        })
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn missing_ops_finishes() {
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let cert = Tx2Cert::arbitrary(&mut u).unwrap();

    let bob = setup_standard_player(ShardedGossipLocalState {
        round_map: maplit::hashmap! {
            cert.clone() => RoundState {
                common_arc_set: Arc::new(ArcInterval::Full.into()),
                num_ops_blooms: 1,
                increment_ops_complete: true,
                created_at: std::time::Instant::now(),
                round_timeout: u32::MAX,
            }
        }
        .into(),
        ..Default::default()
    })
    .await;

    let incoming = ShardedGossipWire::MissingOps(MissingOps {
        ops: vec![],
        finished: true,
    });

    let outgoing = bob.process_incoming(cert.clone(), incoming).await.unwrap();
    assert_eq!(outgoing.len(), 0);

    bob.inner
        .share_mut(|i, _| {
            assert!(i.initiate_tgt.is_none());
            assert_eq!(i.round_map.current_rounds().len(), 0);
            Ok(())
        })
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn missing_ops_doesnt_finish_awaiting_bloom_responses() {
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let cert = Tx2Cert::arbitrary(&mut u).unwrap();

    let bob = setup_standard_player(ShardedGossipLocalState {
        round_map: maplit::hashmap! {
            cert.clone() => RoundState {
                common_arc_set: Arc::new(ArcInterval::Full.into()),
                num_ops_blooms: 1,
                increment_ops_complete: false,
                created_at: std::time::Instant::now(),
                round_timeout: u32::MAX,
            }
        }
        .into(),
        ..Default::default()
    })
    .await;

    let incoming = ShardedGossipWire::MissingOps(MissingOps {
        ops: vec![],
        finished: true,
    });

    let outgoing = bob.process_incoming(cert.clone(), incoming).await.unwrap();
    assert_eq!(outgoing.len(), 0);

    bob.inner
        .share_mut(|i, _| {
            assert!(i.initiate_tgt.is_none());
            assert_eq!(i.round_map.current_rounds().len(), 1);
            Ok(())
        })
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn bloom_response_finishes() {
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let cert = Tx2Cert::arbitrary(&mut u).unwrap();

    let bob = setup_standard_player(ShardedGossipLocalState {
        round_map: maplit::hashmap! {
            cert.clone() => RoundState {
                common_arc_set: Arc::new(ArcInterval::Full.into()),
                num_ops_blooms: 0,
                increment_ops_complete: false,
                created_at: std::time::Instant::now(),
                round_timeout: u32::MAX,
            }
        }
        .into(),
        ..Default::default()
    })
    .await;

    let incoming = ShardedGossipWire::Ops(Ops {
        filter: empty_bloom(),
        finished: true,
    });

    let outgoing = bob.process_incoming(cert.clone(), incoming).await.unwrap();
    assert_eq!(outgoing.len(), 1);

    bob.inner
        .share_mut(|i, _| {
            assert!(i.initiate_tgt.is_none());
            assert_eq!(i.round_map.current_rounds().len(), 0);
            Ok(())
        })
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn bloom_response_doesnt_finish_outstanding_incoming() {
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let cert = Tx2Cert::arbitrary(&mut u).unwrap();

    let bob = setup_standard_player(ShardedGossipLocalState {
        round_map: maplit::hashmap! {
            cert.clone() => RoundState {
                common_arc_set: Arc::new(ArcInterval::Full.into()),
                num_ops_blooms: 1,
                increment_ops_complete: false,
                created_at: std::time::Instant::now(),
                round_timeout: u32::MAX,
            }
        }
        .into(),
        ..Default::default()
    })
    .await;

    let incoming = ShardedGossipWire::Ops(Ops {
        filter: empty_bloom(),
        finished: true,
    });

    let outgoing = bob.process_incoming(cert.clone(), incoming).await.unwrap();
    assert_eq!(outgoing.len(), 1);

    bob.inner
        .share_mut(|i, _| {
            assert!(i.initiate_tgt.is_none());
            assert_eq!(i.round_map.current_rounds().len(), 1);
            Ok(())
        })
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn no_data_still_finishes() {
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let alice_cert = Tx2Cert::arbitrary(&mut u).unwrap();
    let bob_cert = Tx2Cert::arbitrary(&mut u).unwrap();

    let agents = agents();
    let alice = setup_empty_player(ShardedGossipLocalState {
        local_agents: maplit::hashset!(agents[0].clone()),
        round_map: maplit::hashmap! {
            bob_cert.clone() => RoundState {
                common_arc_set: Arc::new(ArcInterval::Full.into()),
                num_ops_blooms: 0,
                increment_ops_complete: false,
                created_at: std::time::Instant::now(),
                round_timeout: u32::MAX,
            }
        }
        .into(),
        ..Default::default()
    })
    .await;

    let bob = setup_empty_player(ShardedGossipLocalState {
        local_agents: maplit::hashset!(agents[1].clone()),
        round_map: maplit::hashmap! {
            alice_cert.clone() => RoundState {
                common_arc_set: Arc::new(ArcInterval::Full.into()),
                num_ops_blooms: 1,
                increment_ops_complete: true,
                created_at: std::time::Instant::now(),
                round_timeout: u32::MAX,
            }
        }
        .into(),
        ..Default::default()
    })
    .await;

    let incoming = ShardedGossipWire::Ops(Ops {
        filter: empty_bloom(),
        finished: true,
    });

    let outgoing = alice
        .process_incoming(bob_cert.clone(), incoming)
        .await
        .unwrap();
    assert_eq!(outgoing.len(), 1);
    let outgoing = bob
        .process_incoming(alice_cert.clone(), outgoing.into_iter().next().unwrap())
        .await
        .unwrap();
    assert_eq!(outgoing.len(), 0);

    alice
        .inner
        .share_mut(|i, _| {
            assert!(i.initiate_tgt.is_none());
            assert_eq!(i.round_map.current_rounds().len(), 0);
            Ok(())
        })
        .unwrap();
    bob.inner
        .share_mut(|i, _| {
            assert!(i.initiate_tgt.is_none());
            assert_eq!(i.round_map.current_rounds().len(), 0);
            Ok(())
        })
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn double_initiate_is_handled() {
    let agents = agents();
    let alice = setup_empty_player(ShardedGossipLocalState {
        local_agents: maplit::hashset!(agents[0].clone()),
        ..Default::default()
    })
    .await;

    let bob = setup_empty_player(ShardedGossipLocalState {
        local_agents: maplit::hashset!(agents[1].clone()),
        ..Default::default()
    })
    .await;

    let (alice_tgt, _, alice_initiate) = alice.try_initiate().await.unwrap().unwrap();
    let (bob_tgt, _, bob_initiate) = bob.try_initiate().await.unwrap().unwrap();
    let bob_cert = alice_tgt.cert();
    let alice_cert = bob_tgt.cert();
    dbg!(&alice_cert);
    dbg!(&bob_cert);
    alice
        .inner
        .share_ref(|i| {
            dbg!(&i.initiate_tgt);
            dbg!(&i.round_map);
            Ok(())
        })
        .unwrap();

    let alice_outgoing = alice
        .process_incoming(bob_cert.clone(), bob_initiate)
        .await
        .unwrap();
    assert_eq!(alice_outgoing.len(), 5);
    let bob_outgoing = bob
        .process_incoming(alice_cert.clone(), alice_initiate)
        .await
        .unwrap();
    dbg!(&bob_outgoing);
    assert_eq!(bob_outgoing.len(), 5);
    todo!()
    // let outgoing = bob
    //     .process_incoming(alice_cert.clone(), outgoing.into_iter().next().unwrap())
    //     .await
    //     .unwrap();
    // assert_eq!(outgoing.len(), 0);
}

async fn agent_info(agent: Arc<KitsuneAgent>) -> AgentInfoSigned {
    AgentInfoSigned::sign(
            Arc::new(fixt!(KitsuneSpace)),
            agent,
            u32::MAX / 4,
            vec![url2::url2!("kitsune-proxy://CIW6PxKxsPPlcuvUCbMcKwUpaMSmB7kLD8xyyj4mqcw/kitsune-quic/h/localhost/p/5778/-").into()],
            0,
            0,
            |_| async move { Ok(Arc::new(fixt!(KitsuneSignature, Predictable))) },
        )
        .await
        .unwrap()
}

fn empty_bloom() -> Option<PoolBuf> {
    let bloom = bloomfilter::Bloom::new_for_fp_rate(1, 0.1);
    let bloom = TimedBloomFilter {
        bloom,
        time: 0..u64::MAX,
    };
    Some(encode_timed_bloom_filter(&bloom))
}
