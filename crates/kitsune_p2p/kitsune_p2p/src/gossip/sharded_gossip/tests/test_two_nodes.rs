use super::common::*;
use super::*;
use crate::NOISE;
use arbitrary::Arbitrary;

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

    let mut agents = agents(2).into_iter();
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

    let agents = agents(2);
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
    let agents = agents(2);
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
