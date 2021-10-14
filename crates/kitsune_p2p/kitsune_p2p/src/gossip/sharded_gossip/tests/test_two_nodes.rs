use super::common::*;
use super::*;
use crate::NOISE;
use arbitrary::Arbitrary;

#[tokio::test(flavor = "multi_thread")]
/// Runs through a happy path gossip round between two agents.
async fn sharded_sanity_test() {
    // - Setup players and data.
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

    // - Bob try's to initiate.
    let (_, _, bob_outgoing) = bob.try_initiate().await.unwrap().unwrap();
    let alices_cert = bob
        .inner
        .share_ref(|i| Ok(i.initiate_tgt.as_ref().unwrap().0.cert().clone()))
        .unwrap();

    // - Send initiate to alice.
    let alice_outgoing = alice
        .process_incoming(bob_cert.clone(), bob_outgoing)
        .await
        .unwrap();

    // - Alice responds to the initiate with 1 accept and 4 blooms.
    assert_eq!(alice_outgoing.len(), 5);
    alice
        .inner
        .share_mut(|i, _| {
            // - Check alice has one current round.
            assert_eq!(i.round_map.current_rounds().len(), 1);
            Ok(())
        })
        .unwrap();

    let mut bob_outgoing = Vec::new();

    // - Send the above to bob.
    for incoming in alice_outgoing {
        let outgoing = bob
            .process_incoming(alices_cert.clone(), incoming)
            .await
            .unwrap();
        bob_outgoing.extend(outgoing);
    }

    // - Bob responds with 4 blooms and 4 responses to alice's blooms.
    assert_eq!(bob_outgoing.len(), 8);
    bob.inner
        .share_mut(|i, _| {
            // - Check bob has one current round.
            assert_eq!(i.round_map.current_rounds().len(), 1);
            Ok(())
        })
        .unwrap();

    let mut alice_outgoing = Vec::new();

    // - Send the above to alice.
    for incoming in bob_outgoing {
        let outgoing = alice
            .process_incoming(bob_cert.clone(), incoming)
            .await
            .unwrap();
        alice_outgoing.extend(outgoing);
    }
    // - Alice responds with 4 responses to bob's blooms.
    assert_eq!(alice_outgoing.len(), 4);

    alice
        .inner
        .share_mut(|i, _| {
            // Assert alice has no initiate target.
            assert!(i.initiate_tgt.is_none());
            // Assert alice has no current rounds as alice
            // has now finished this round of gossip.
            assert_eq!(i.round_map.current_rounds().len(), 0);
            Ok(())
        })
        .unwrap();

    let mut bob_outgoing = Vec::new();
    // - Send alice's missing ops messages to bob.
    for incoming in alice_outgoing {
        let outgoing = bob
            .process_incoming(alices_cert.clone(), incoming)
            .await
            .unwrap();
        bob_outgoing.extend(outgoing);
    }
    // - Bob should have no responses.
    assert_eq!(bob_outgoing.len(), 0);

    bob.inner
        .share_mut(|i, _| {
            // Assert bob has no initiate target.
            assert!(i.initiate_tgt.is_none());
            // Assert bob has no current rounds as alice
            // has now finished this round of gossip.
            assert_eq!(i.round_map.current_rounds().len(), 0);
            Ok(())
        })
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
/// This tests that sending missing ops that isn't
/// marked as finished does not finish the round.
async fn partial_missing_doesnt_finish() {
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let cert = Tx2Cert::arbitrary(&mut u).unwrap();

    // - Set bob up with a current round that expects one
    // response to a sent bloom.
    let bob = setup_standard_player(ShardedGossipLocalState {
        round_map: maplit::hashmap! {
            cert.clone() => RoundState {
                common_arc_set: Arc::new(ArcInterval::Full.into()),
                num_sent_ops_blooms: 1,
                received_all_incoming_ops_blooms: true,
                created_at: Instant::now(),
                last_touch: Instant::now(),
                round_timeout: std::time::Duration::MAX,
            }
        }
        .into(),
        ..Default::default()
    })
    .await;

    // - Send a missing ops message that isn't marked as finished.
    let incoming = ShardedGossipWire::MissingOps(MissingOps {
        ops: vec![],
        finished: false,
    });

    let outgoing = bob.process_incoming(cert.clone(), incoming).await.unwrap();
    assert_eq!(outgoing.len(), 0);

    bob.inner
        .share_mut(|i, _| {
            assert!(i.initiate_tgt.is_none());
            // - Check bob still has a current round.
            assert_eq!(i.round_map.current_rounds().len(), 1);
            Ok(())
        })
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
/// This test checks that a missing ops message that is
/// marked as finished does finish the round.
async fn missing_ops_finishes() {
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let cert = Tx2Cert::arbitrary(&mut u).unwrap();

    // - Set bob up the same as the test above.
    let bob = setup_standard_player(ShardedGossipLocalState {
        round_map: maplit::hashmap! {
            cert.clone() => RoundState {
                common_arc_set: Arc::new(ArcInterval::Full.into()),
                num_sent_ops_blooms: 1,
                received_all_incoming_ops_blooms: true,
                created_at: Instant::now(),
                last_touch: Instant::now(),
                round_timeout: std::time::Duration::MAX,
            }
        }
        .into(),
        ..Default::default()
    })
    .await;

    // Send a message marked as finished.
    let incoming = ShardedGossipWire::MissingOps(MissingOps {
        ops: vec![],
        finished: true,
    });

    let outgoing = bob.process_incoming(cert.clone(), incoming).await.unwrap();
    assert_eq!(outgoing.len(), 0);

    bob.inner
        .share_mut(|i, _| {
            assert!(i.initiate_tgt.is_none());
            // - Bob now has no current rounds.
            assert_eq!(i.round_map.current_rounds().len(), 0);
            Ok(())
        })
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
/// This test checks that a missing ops message that is
/// marked as finished doesn't finish the round when
/// the player is still awaiting incoming blooms.
async fn missing_ops_doesnt_finish_awaiting_bloom_responses() {
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let cert = Tx2Cert::arbitrary(&mut u).unwrap();

    // - Set bob up awaiting incoming blooms and one response.
    let bob = setup_standard_player(ShardedGossipLocalState {
        round_map: maplit::hashmap! {
            cert.clone() => RoundState {
                common_arc_set: Arc::new(ArcInterval::Full.into()),
                num_sent_ops_blooms: 1,
                received_all_incoming_ops_blooms: false,
                created_at: Instant::now(),
                last_touch: Instant::now(),
                round_timeout: std::time::Duration::MAX,
            }
        }
        .into(),
        ..Default::default()
    })
    .await;

    // - Send a message marked as finished.
    let incoming = ShardedGossipWire::MissingOps(MissingOps {
        ops: vec![],
        finished: true,
    });

    let outgoing = bob.process_incoming(cert.clone(), incoming).await.unwrap();
    assert_eq!(outgoing.len(), 0);

    bob.inner
        .share_mut(|i, _| {
            assert!(i.initiate_tgt.is_none());
            // - Bob still has a current round.
            assert_eq!(i.round_map.current_rounds().len(), 1);
            Ok(())
        })
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
/// This test checks that a ops bloom message does
/// finish the round when there are no outstanding response.
async fn bloom_response_finishes() {
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let cert = Tx2Cert::arbitrary(&mut u).unwrap();

    // - Set bob up with a current round that expects no responses
    // and has not received all blooms.
    let bob = setup_standard_player(ShardedGossipLocalState {
        round_map: maplit::hashmap! {
            cert.clone() => RoundState {
                common_arc_set: Arc::new(ArcInterval::Full.into()),
                num_sent_ops_blooms: 0,
                received_all_incoming_ops_blooms: false,
                created_at: Instant::now(),
                last_touch: Instant::now(),
                round_timeout: std::time::Duration::MAX,
            }
        }
        .into(),
        ..Default::default()
    })
    .await;

    // - Send the final ops bloom message.
    let incoming = ShardedGossipWire::Ops(Ops {
        missing_hashes: empty_bloom(),
        finished: true,
    });

    let outgoing = bob.process_incoming(cert.clone(), incoming).await.unwrap();
    assert_eq!(outgoing.len(), 1);

    bob.inner
        .share_mut(|i, _| {
            assert!(i.initiate_tgt.is_none());
            // - Bob now has no current rounds.
            assert_eq!(i.round_map.current_rounds().len(), 0);
            Ok(())
        })
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
/// This test checks that an ops bloom message doesn't
/// finish the round when their are outstanding responses.
async fn bloom_response_doesnt_finish_outstanding_incoming() {
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let cert = Tx2Cert::arbitrary(&mut u).unwrap();

    // - Set bob up with a current round that expects one response
    // and has not received all blooms.
    let bob = setup_standard_player(ShardedGossipLocalState {
        round_map: maplit::hashmap! {
            cert.clone() => RoundState {
                common_arc_set: Arc::new(ArcInterval::Full.into()),
                num_sent_ops_blooms: 1,
                received_all_incoming_ops_blooms: false,
                created_at: Instant::now(),
                last_touch: Instant::now(),
                round_timeout: std::time::Duration::MAX,
            }
        }
        .into(),
        ..Default::default()
    })
    .await;

    // - Send the final ops bloom message.
    let incoming = ShardedGossipWire::Ops(Ops {
        missing_hashes: empty_bloom(),
        finished: true,
    });

    let outgoing = bob.process_incoming(cert.clone(), incoming).await.unwrap();
    assert_eq!(outgoing.len(), 1);

    bob.inner
        .share_mut(|i, _| {
            assert!(i.initiate_tgt.is_none());
            // - Bob still has a current round.
            assert_eq!(i.round_map.current_rounds().len(), 1);
            Ok(())
        })
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
/// This test checks that a round with no data can
/// still finish.
async fn no_data_still_finishes() {
    // - Set up two players with no data.
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let alice_cert = Tx2Cert::arbitrary(&mut u).unwrap();
    let bob_cert = Tx2Cert::arbitrary(&mut u).unwrap();

    let agents = agents(2);
    // - Alice is expecting no responses and is expecting blooms.
    let alice = setup_empty_player(ShardedGossipLocalState {
        local_agents: maplit::hashset!(agents[0].clone()),
        round_map: maplit::hashmap! {
            bob_cert.clone() => RoundState {
                common_arc_set: Arc::new(ArcInterval::Full.into()),
                num_sent_ops_blooms: 0,
                received_all_incoming_ops_blooms: false,
                created_at: Instant::now(),
                last_touch: Instant::now(),
                round_timeout: std::time::Duration::MAX,
            }
        }
        .into(),
        ..Default::default()
    })
    .await;

    // - Bob is expecting one responses and is expecting no blooms.
    let bob = setup_empty_player(ShardedGossipLocalState {
        local_agents: maplit::hashset!(agents[1].clone()),
        round_map: maplit::hashmap! {
            alice_cert.clone() => RoundState {
                common_arc_set: Arc::new(ArcInterval::Full.into()),
                num_sent_ops_blooms: 1,
                received_all_incoming_ops_blooms: true,
                created_at: Instant::now(),
                last_touch: Instant::now(),
                round_timeout: std::time::Duration::MAX,
            }
        }
        .into(),
        ..Default::default()
    })
    .await;

    // - Send the final ops bloom message to alice.
    let incoming = ShardedGossipWire::Ops(Ops {
        missing_hashes: empty_bloom(),
        finished: true,
    });

    let outgoing = alice
        .process_incoming(bob_cert.clone(), incoming)
        .await
        .unwrap();

    // - Alice responds with an empty missing ops.
    assert_eq!(outgoing.len(), 1);

    // - Send this to bob.
    let outgoing = bob
        .process_incoming(alice_cert.clone(), outgoing.into_iter().next().unwrap())
        .await
        .unwrap();

    // - Bob has no response.
    assert_eq!(outgoing.len(), 0);

    // - Both players have no current rounds.
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
/// This test checks that when two players concurrently
/// initiate a round it is handle correctly.
async fn double_initiate_is_handled() {
    let agents = agents(2);
    // - Set up two players with themselves as local agents.
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

    // - Both players try to initiate and only have the other as a remote agent.
    let (alice_tgt, _, alice_initiate) = alice.try_initiate().await.unwrap().unwrap();
    let (bob_tgt, _, bob_initiate) = bob.try_initiate().await.unwrap().unwrap();
    let bob_cert = alice_tgt.cert();
    let alice_cert = bob_tgt.cert();

    // - Both players process the initiate.
    let alice_outgoing = alice
        .process_incoming(bob_cert.clone(), bob_initiate)
        .await
        .unwrap();
    let bob_outgoing = bob
        .process_incoming(alice_cert.clone(), alice_initiate)
        .await
        .unwrap();

    // - Check we always have at least one node not proceeding with initiate.
    assert!((bob_outgoing.len() == 0 || alice_outgoing.len() == 0));
}

#[tokio::test(flavor = "multi_thread")]
/// This test checks that trying to initiate after a round with
/// a node is already in progress does not initiate a new round.
async fn initiate_after_target_is_set() {
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

    // - Alice successfully initiates a round with bob.
    let (tgt, _, alice_initiate) = alice.try_initiate().await.unwrap().unwrap();
    let cert = tgt.cert();
    // - Bob accepts the round.
    let bob_outgoing = bob
        .process_incoming(cert.clone(), alice_initiate)
        .await
        .unwrap();
    assert_eq!(bob_outgoing.len(), 5);

    // - Bob tries to initiate a round with alice.
    let bob_initiate = bob.try_initiate().await.unwrap();
    // - Bob cannot initiate a round with anyone because he
    // already has a round with the only other player.
    assert!(bob_initiate.is_none());
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
/// Test the initiates timeout after the round timeout has elapsed.
async fn initiate_times_out() {
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let alice_cert = Tx2Cert::arbitrary(&mut u).unwrap();

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

    // Trying to initiate a round should succeed.
    let (tgt, _, _) = alice
        .try_initiate()
        .await
        .unwrap()
        .expect("Failed to initiate");
    alice
        .inner
        .share_mut(|i, _| {
            assert!(i.initiate_tgt.is_some());
            Ok(())
        })
        .unwrap();
    let r = alice.try_initiate().await.unwrap();

    // Doesn't re-initiate.
    assert!(r.is_none());
    alice
        .inner
        .share_mut(|i, _| {
            assert!(i.initiate_tgt.is_some());
            Ok(())
        })
        .unwrap();

    // Wait slightly longer then the timeout.
    tokio::time::sleep(ROUND_TIMEOUT + std::time::Duration::from_millis(1)).await;

    let (tgt2, _, alice_initiate) = alice
        .try_initiate()
        .await
        .unwrap()
        .expect("Failed to initiate");

    // Now it should re-initiate with a different node.
    assert_ne!(tgt, tgt2);
    alice
        .inner
        .share_mut(|i, _| {
            assert!(i.initiate_tgt.is_some());
            Ok(())
        })
        .unwrap();

    // Process the initiate with Bob.
    let bob_outgoing = bob
        .process_incoming(alice_cert.clone(), alice_initiate)
        .await
        .unwrap();

    // Process the Bob's accept with Alice.
    for bo in bob_outgoing {
        alice
            .process_incoming(tgt2.cert().clone(), bo)
            .await
            .unwrap();
    }

    // Check the round is now active.
    alice
        .inner
        .share_mut(|i, _| {
            assert!(i.initiate_tgt.is_some());
            assert_eq!(i.round_map.current_rounds().len(), 1);
            Ok(())
        })
        .unwrap();

    // Wait slightly longer then the timeout but touch the round in between.
    tokio::time::sleep(ROUND_TIMEOUT / 2).await;

    // Get the map so the round doesn't timeout
    alice
        .inner
        .share_mut(|i, _| {
            i.round_map.get(tgt2.cert());
            Ok(())
        })
        .unwrap();

    tokio::time::sleep(ROUND_TIMEOUT / 2 + std::time::Duration::from_millis(1)).await;

    // Check that initiating again doesn't do anything.

    let r = alice.try_initiate().await.unwrap();
    // Doesn't re-initiate.
    assert!(r.is_none());
    alice
        .inner
        .share_mut(|i, _| {
            assert!(i.initiate_tgt.is_some());
            Ok(())
        })
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
/// Runs through a happy path gossip round between two agents.
async fn gossips_empty_info() {
    // - Setup players and data.
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let bob_cert = Tx2Cert::arbitrary(&mut u).unwrap();

    // - Create some agents.
    let mut agents = agents(4).into_iter();
    let alice_agent = agents.next().unwrap();
    let bob_agent = agents.next().unwrap();
    let empty_agent_1 = agents.next().unwrap();
    let empty_agent_2 = agents.next().unwrap();

    // - Get their agent info.
    let alice_info = agent_info(alice_agent.clone()).await;
    let bob_info = agent_info(bob_agent.clone()).await;
    let empty_1_info = empty_agent_info(empty_agent_1.clone(), alice_info.url_list.clone()).await;
    let empty_2_info = empty_agent_info(empty_agent_2.clone(), bob_info.url_list.clone()).await;
    let peer_data = vec![alice_info.clone(), bob_info.clone(), empty_1_info.clone()];

    // - Setup alice's conductor.
    let mut evt_handler = MockKitsuneP2pEventHandler::new();
    evt_handler.expect_handle_query_agents().returning({
        let peer_data = peer_data.clone();
        move |a| {
            let peer_data = peer_data.clone();
            Ok(async move { Ok(peer_data) }.boxed().into())
        }
    });
    evt_handler
        .expect_handle_query_op_hashes()
        .returning(|_| Ok(async { Ok(None) }.boxed().into()));
    evt_handler
        .expect_handle_put_agent_info_signed()
        // - Assert alice get's only the empty agent from bob's node.
        .withf({
            let empty_2_info = empty_2_info.clone();
            move |info| info.peer_data == vec![empty_2_info.clone()]
        })
        .returning(|_| Ok(async { Ok(()) }.boxed().into()));
    evt_handler
        .expect_handle_gossip()
        .returning(|_, _, _| Ok(async { Ok(()) }.boxed().into()));

    // - Setup alice's local state.
    let state = ShardedGossipLocalState {
        local_agents: maplit::hashset! { alice_agent.clone(), empty_agent_1.clone() },
        ..Default::default()
    };
    let (evt_sender, _) = spawn_handler(evt_handler).await;
    let alice = ShardedGossipLocal::test(GossipType::Recent, evt_sender, state);

    // - Setup bob's conductor.
    let mut evt_handler = MockKitsuneP2pEventHandler::new();
    let peer_data = vec![alice_info.clone(), bob_info.clone(), empty_2_info.clone()];
    evt_handler.expect_handle_query_agents().returning({
        let peer_data = peer_data.clone();
        move |_| {
            let peer_data = peer_data.clone();
            Ok(async move { Ok(peer_data) }.boxed().into())
        }
    });
    evt_handler
        .expect_handle_query_op_hashes()
        .returning(|_| Ok(async { Ok(None) }.boxed().into()));
    evt_handler
        .expect_handle_put_agent_info_signed()
        // - Assert bob get's only the empty agent from alice's node.
        .withf({
            let empty_1_info = empty_1_info.clone();
            move |info| info.peer_data == vec![empty_1_info.clone()]
        })
        .returning(|_| Ok(async { Ok(()) }.boxed().into()));
    evt_handler
        .expect_handle_gossip()
        .returning(|_, _, _| Ok(async { Ok(()) }.boxed().into()));

    // - Setup bob's local state.
    let state = ShardedGossipLocalState {
        local_agents: maplit::hashset! { bob_agent.clone(), empty_agent_2.clone() },
        ..Default::default()
    };
    let (evt_sender, _) = spawn_handler(evt_handler).await;
    let bob = ShardedGossipLocal::test(GossipType::Recent, evt_sender, state);

    // - Bob try's to initiate.
    let (_, _, bob_outgoing) = bob.try_initiate().await.unwrap().unwrap();
    let alices_cert = bob
        .inner
        .share_ref(|i| Ok(i.initiate_tgt.as_ref().unwrap().0.cert().clone()))
        .unwrap();

    // - Send initiate to alice.
    let alice_outgoing = alice
        .process_incoming(bob_cert.clone(), bob_outgoing)
        .await
        .unwrap();

    // - Alice responds to the initiate with 1 accept and 2 blooms.
    assert_eq!(alice_outgoing.len(), 3);

    let mut bob_outgoing = Vec::new();

    // - Send the above to bob.
    for incoming in alice_outgoing {
        let outgoing = bob
            .process_incoming(alices_cert.clone(), incoming)
            .await
            .unwrap();
        bob_outgoing.extend(outgoing);
    }

    // - Bob responds with 2 blooms and 2 responses to alice's blooms.
    assert_eq!(bob_outgoing.len(), 4);

    let mut alice_outgoing = Vec::new();

    // - Send the above to bob.
    for incoming in bob_outgoing {
        let outgoing = alice
            .process_incoming(bob_cert.clone(), incoming)
            .await
            .unwrap();
        alice_outgoing.extend(outgoing);
    }

    assert_eq!(alice_outgoing.len(), 2);

    let mut bob_outgoing = Vec::new();

    // - Send the above to bob.
    for incoming in alice_outgoing {
        let outgoing = bob
            .process_incoming(alices_cert.clone(), incoming)
            .await
            .unwrap();
        bob_outgoing.extend(outgoing);
    }

    assert_eq!(bob_outgoing.len(), 0);
}
