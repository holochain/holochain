use super::common::*;
use super::*;
use crate::NOISE;
use arbitrary::Arbitrary;
use itertools::Itertools;

#[tokio::test(flavor = "multi_thread")]
/// Runs through a happy path gossip round between two agents.
async fn sharded_sanity_test() {
    // - Setup players and data.
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let bob_cert = Tx2Cert::arbitrary(&mut u).unwrap();

    let agents = agents_with_infos(2).await;
    let all_agents: Vec<AgentInfoSigned> = agents.iter().map(|x| x.1.clone()).collect();
    let mut iter = agents.clone().into_iter();
    let alice_agent = iter.next().unwrap().0;
    let bob_agent = iter.next().unwrap().0;

    let alice = setup_standard_player(
        ShardedGossipLocalState {
            local_agents: maplit::hashset! { alice_agent.clone() },
            ..Default::default()
        },
        agents.clone(),
    )
    .await;

    let bob = setup_standard_player(
        ShardedGossipLocalState {
            local_agents: maplit::hashset! { bob_agent.clone() },
            ..Default::default()
        },
        agents.clone(),
    )
    .await;

    let mut agent_info_session = AgentInfoSession::new(
        bob.query_agents_by_local_agents().await.unwrap(),
        all_agents.clone(),
    );

    // - Bob tries to initiate.
    let (_, _, bob_outgoing) = bob
        .try_initiate(&mut agent_info_session)
        .await
        .unwrap()
        .unwrap();
    let alices_cert = bob
        .inner
        .share_ref(|i| Ok(i.initiate_tgt.as_ref().unwrap().cert.clone()))
        .unwrap();

    // - Send initiate to alice.
    let alice_outgoing = alice
        .process_incoming(
            bob_cert.clone().into(),
            bob_outgoing,
            &mut agent_info_session,
        )
        .await
        .unwrap();

    // - Alice responds to the initiate with 1 accept and 1 blooms.
    assert_eq!(alice_outgoing.len(), 2);
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
            .process_incoming(alices_cert.clone(), incoming, &mut agent_info_session)
            .await
            .unwrap();
        bob_outgoing.extend(outgoing);
    }

    // - Bob responds with 1 blooms and 1 responses to alice's blooms.
    assert_eq!(bob_outgoing.len(), 2);
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
            .process_incoming(bob_cert.clone().into(), incoming, &mut agent_info_session)
            .await
            .unwrap();
        alice_outgoing.extend(outgoing);
    }
    // - Alice responds with 1 responses to bob's blooms.
    assert_eq!(alice_outgoing.len(), 1);

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
            .process_incoming(alices_cert.clone(), incoming, &mut agent_info_session)
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
    let bob = setup_standard_player(
        ShardedGossipLocalState {
            round_map: maplit::hashmap! {
                cert.clone().into() => RoundState {
                    remote_agent_list: vec![],
                    common_arc_set: Arc::new(DhtArcSet::Full),
                    num_expected_op_blooms: 1,
                    received_all_incoming_op_blooms: true,
                    has_pending_historical_op_data: false,
                    regions_are_queued: true,
                    id: nanoid::nanoid!(),
                    last_touch: Instant::now(),
                    round_timeout: std::time::Duration::MAX,
                    bloom_batch_cursor: None,
                    ops_batch_queue: OpsBatchQueue::new(),
                    region_set_sent: None,
                    region_diffs: Default::default(),
                }
            }
            .into(),
            ..Default::default()
        },
        vec![],
    )
    .await;

    // - Send a missing ops message that isn't marked as finished.
    let incoming = ShardedGossipWire::MissingOpHashes(MissingOpHashes {
        ops: vec![],
        finished: MissingOpsStatus::ChunkComplete as u8,
    });

    let mut agent_info_session =
        AgentInfoSession::new(bob.query_agents_by_local_agents().await.unwrap(), vec![]);

    let outgoing = bob
        .process_incoming(cert.clone().into(), incoming, &mut agent_info_session)
        .await
        .unwrap();
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
    let bob = setup_standard_player(
        ShardedGossipLocalState {
            round_map: maplit::hashmap! {
                cert.clone().into() => RoundState {
                    remote_agent_list: vec![],
                    common_arc_set: Arc::new(DhtArcSet::Full),
                    num_expected_op_blooms: 1,
                    received_all_incoming_op_blooms: true,
                    has_pending_historical_op_data: false,
                    regions_are_queued: true,
                    id: nanoid::nanoid!(),
                    last_touch: Instant::now(),
                    round_timeout: std::time::Duration::MAX,
                    bloom_batch_cursor: None,
                    ops_batch_queue: OpsBatchQueue::new(),
                    region_set_sent: None,
                    region_diffs: Default::default(),
                }
            }
            .into(),
            ..Default::default()
        },
        vec![],
    )
    .await;

    // Send a message marked as finished.
    let incoming = ShardedGossipWire::MissingOpHashes(MissingOpHashes {
        ops: vec![],
        finished: MissingOpsStatus::AllComplete as u8,
    });

    let mut agent_info_session =
        AgentInfoSession::new(bob.query_agents_by_local_agents().await.unwrap(), vec![]);

    let outgoing = bob
        .process_incoming(cert.clone().into(), incoming, &mut agent_info_session)
        .await
        .unwrap();
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
    let bob = setup_standard_player(
        ShardedGossipLocalState {
            round_map: maplit::hashmap! {
                cert.clone().into() => RoundState {
                    remote_agent_list: vec![],
                    common_arc_set: Arc::new(DhtArcSet::Full),
                    num_expected_op_blooms: 1,
                    received_all_incoming_op_blooms: false,
                    has_pending_historical_op_data: false,
                    regions_are_queued: true,
                    id: nanoid::nanoid!(),
                    last_touch: Instant::now(),
                    round_timeout: std::time::Duration::MAX,
                    bloom_batch_cursor: None,
                    ops_batch_queue: OpsBatchQueue::new(),
                    region_set_sent: None,
                    region_diffs: Default::default(),
                }
            }
            .into(),
            ..Default::default()
        },
        vec![],
    )
    .await;

    // - Send a message marked as finished.
    let incoming = ShardedGossipWire::MissingOpHashes(MissingOpHashes {
        ops: vec![],
        finished: MissingOpsStatus::AllComplete as u8,
    });

    let mut agent_info_session =
        AgentInfoSession::new(bob.query_agents_by_local_agents().await.unwrap(), vec![]);

    let outgoing = bob
        .process_incoming(cert.clone().into(), incoming, &mut agent_info_session)
        .await
        .unwrap();
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
    let bob = setup_standard_player(
        ShardedGossipLocalState {
            round_map: maplit::hashmap! {
                cert.clone().into() => RoundState {
                    remote_agent_list: vec![],
                    common_arc_set: Arc::new(DhtArcSet::Full),
                    num_expected_op_blooms: 0,
                    received_all_incoming_op_blooms: false,
                    has_pending_historical_op_data: false,
                    regions_are_queued: true,
                    id: nanoid::nanoid!(),
                    last_touch: Instant::now(),
                    round_timeout: std::time::Duration::MAX,
                    bloom_batch_cursor: None,
                    ops_batch_queue: OpsBatchQueue::new(),
                    region_set_sent: None,
                    region_diffs: Default::default(),
                }
            }
            .into(),
            ..Default::default()
        },
        vec![],
    )
    .await;

    // - Send the final ops bloom message.
    let incoming = ShardedGossipWire::OpBloom(OpBloom {
        missing_hashes: empty_bloom(),
        finished: true,
    });

    let mut agent_info_session =
        AgentInfoSession::new(bob.query_agents_by_local_agents().await.unwrap(), vec![]);

    let outgoing = bob
        .process_incoming(cert.clone().into(), incoming, &mut agent_info_session)
        .await
        .unwrap();
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
    let bob = setup_standard_player(
        ShardedGossipLocalState {
            round_map: maplit::hashmap! {
                cert.clone().into() => RoundState {
                    remote_agent_list: vec![],
                    common_arc_set: Arc::new(DhtArcSet::Full),
                    num_expected_op_blooms: 1,
                    received_all_incoming_op_blooms: false,
                    has_pending_historical_op_data: false,
                    regions_are_queued: true,
                    id: nanoid::nanoid!(),
                    last_touch: Instant::now(),
                    round_timeout: std::time::Duration::MAX,
                    bloom_batch_cursor: None,
                    ops_batch_queue: OpsBatchQueue::new(),
                    region_set_sent: None,
                    region_diffs: Default::default(),
                }
            }
            .into(),
            ..Default::default()
        },
        vec![],
    )
    .await;

    // - Send the final ops bloom message.
    let incoming = ShardedGossipWire::OpBloom(OpBloom {
        missing_hashes: empty_bloom(),
        finished: true,
    });

    let mut agent_info_session =
        AgentInfoSession::new(bob.query_agents_by_local_agents().await.unwrap(), vec![]);

    let outgoing = bob
        .process_incoming(cert.clone().into(), incoming, &mut agent_info_session)
        .await
        .unwrap();
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

    let agents = agents_with_infos(2).await;
    let all_agents = agents.iter().map(|(_, info)| info.clone()).collect_vec();
    // - Alice is expecting no responses and is expecting blooms.
    let alice = setup_empty_player(
        ShardedGossipLocalState {
            local_agents: maplit::hashset!(agents[0].0.clone()),
            round_map: maplit::hashmap! {
                bob_cert.clone().into() => RoundState {
                    remote_agent_list: vec![],
                    common_arc_set: Arc::new(DhtArcSet::Full),
                    num_expected_op_blooms: 0,
                    received_all_incoming_op_blooms: false,
                    has_pending_historical_op_data: false,
                    regions_are_queued: true,
                    id: nanoid::nanoid!(),
                    last_touch: Instant::now(),
                    round_timeout: std::time::Duration::MAX,
                    bloom_batch_cursor: None,
                    ops_batch_queue: OpsBatchQueue::new(),
                    region_set_sent: None,
                    region_diffs: Default::default(),
                }
            }
            .into(),
            ..Default::default()
        },
        agents.clone(),
    )
    .await;

    // - Bob is expecting one responses and is expecting no blooms.
    let bob = setup_empty_player(
        ShardedGossipLocalState {
            local_agents: maplit::hashset!(agents[1].0.clone()),
            round_map: maplit::hashmap! {
                alice_cert.clone().into() => RoundState {
                    remote_agent_list: vec![],
                    common_arc_set: Arc::new(DhtArcSet::Full),
                    num_expected_op_blooms: 1,
                    received_all_incoming_op_blooms: true,
                    has_pending_historical_op_data: false,
                    regions_are_queued: true,
                    id: nanoid::nanoid!(),
                    last_touch: Instant::now(),
                    round_timeout: std::time::Duration::MAX,
                    bloom_batch_cursor: None,
                    ops_batch_queue: OpsBatchQueue::new(),
                    region_set_sent: None,
                    region_diffs: Default::default(),
                }
            }
            .into(),
            ..Default::default()
        },
        agents.clone(),
    )
    .await;

    // - Send the final ops bloom message to alice.
    let incoming = ShardedGossipWire::OpBloom(OpBloom {
        missing_hashes: empty_bloom(),
        finished: true,
    });

    let outgoing = alice
        .process_incoming(
            bob_cert.clone().into(),
            incoming,
            &mut AgentInfoSession::new(
                alice.query_agents_by_local_agents().await.unwrap(),
                all_agents.clone(),
            ),
        )
        .await
        .unwrap();

    // - Alice responds with an empty missing ops.
    assert_eq!(outgoing.len(), 1);

    // - Send this to bob.
    let outgoing = bob
        .process_incoming(
            alice_cert.clone().into(),
            outgoing.into_iter().next().unwrap(),
            &mut AgentInfoSession::new(
                bob.query_agents_by_local_agents().await.unwrap(),
                all_agents.clone(),
            ),
        )
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
/// This test checks that when two players simultaneously
/// initiate a round it is handled correctly.
async fn double_initiate_is_handled() {
    let agents = agents_with_infos(2).await;
    let all_agents: Vec<AgentInfoSigned> = agents.iter().map(|x| x.1.clone()).collect();
    // - Set up two players with themselves as local agents.
    let alice = setup_empty_player(
        ShardedGossipLocalState {
            local_agents: maplit::hashset!(agents[0].0.clone()),
            ..Default::default()
        },
        agents.clone(),
    )
    .await;

    let bob = setup_empty_player(
        ShardedGossipLocalState {
            local_agents: maplit::hashset!(agents[1].0.clone()),
            ..Default::default()
        },
        agents.clone(),
    )
    .await;

    // - Both players try to initiate and only have the other as a remote agent.
    let (bob_cert, _, alice_initiate) = alice
        .try_initiate(&mut AgentInfoSession::new(
            alice.query_agents_by_local_agents().await.unwrap(),
            all_agents.clone(),
        ))
        .await
        .unwrap()
        .unwrap();
    let (alice_cert, _, bob_initiate) = bob
        .try_initiate(&mut AgentInfoSession::new(
            bob.query_agents_by_local_agents().await.unwrap(),
            all_agents.clone(),
        ))
        .await
        .unwrap()
        .unwrap();

    // - Both players process the initiate.
    let alice_outgoing = alice
        .process_incoming(
            bob_cert,
            bob_initiate,
            &mut AgentInfoSession::new(
                alice.query_agents_by_local_agents().await.unwrap(),
                all_agents.clone(),
            ),
        )
        .await
        .unwrap();
    let bob_outgoing = bob
        .process_incoming(
            alice_cert,
            alice_initiate,
            &mut AgentInfoSession::new(
                bob.query_agents_by_local_agents().await.unwrap(),
                all_agents.clone(),
            ),
        )
        .await
        .unwrap();

    // - Check we always have at least one node not proceeding with initiate.
    assert!((bob_outgoing.is_empty() || alice_outgoing.is_empty()));
}

#[tokio::test(flavor = "multi_thread")]
/// This test checks that trying to initiate after a round with
/// a node is already in progress does not initiate a new round.
async fn initiate_after_target_is_set() {
    let agents = agents_with_infos(2).await;
    let all_agents: Vec<AgentInfoSigned> = agents.iter().map(|x| x.1.clone()).collect();
    let alice = setup_empty_player(
        ShardedGossipLocalState {
            local_agents: maplit::hashset!(agents[0].0.clone()),
            ..Default::default()
        },
        agents.clone(),
    )
    .await;

    let bob = setup_empty_player(
        ShardedGossipLocalState {
            local_agents: maplit::hashset!(agents[1].0.clone()),
            ..Default::default()
        },
        agents.clone(),
    )
    .await;

    // - Alice successfully initiates a round with bob.
    let (cert, _, alice_initiate) = alice
        .try_initiate(&mut AgentInfoSession::new(
            alice.query_agents_by_local_agents().await.unwrap(),
            all_agents.clone(),
        ))
        .await
        .unwrap()
        .unwrap();
    dbg!(&cert);
    dbg!(&agents);
    // - Bob accepts the round.
    let bob_outgoing = bob
        .process_incoming(
            cert.clone(),
            alice_initiate,
            &mut AgentInfoSession::new(
                bob.query_agents_by_local_agents().await.unwrap(),
                all_agents.clone(),
            ),
        )
        .await
        .unwrap();
    assert_eq!(bob_outgoing.len(), 2);

    bob.inner
        .share_mut(|i, _| {
            dbg!(&i.initiate_tgt);
            dbg!(i.round_map.current_rounds().len());
            Ok(())
        })
        .unwrap();
    // - Bob tries to initiate a round with alice.
    let bob_initiate = bob
        .try_initiate(&mut AgentInfoSession::new(
            bob.query_agents_by_local_agents().await.unwrap(),
            all_agents.clone(),
        ))
        .await
        .unwrap();
    bob.inner
        .share_mut(|i, _| {
            dbg!(&i.initiate_tgt);
            dbg!(i.round_map.current_rounds().len());
            Ok(())
        })
        .unwrap();
    // - Bob cannot initiate a round with anyone because he
    // already has a round with the only other player.
    assert!(bob_initiate.is_none());
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
/// Test the initiates timeout after the round timeout has elapsed.
async fn initiate_times_out() {
    holochain_trace::test_run().unwrap();

    let agents = agents_with_infos(3).await;
    let all_agents: Vec<AgentInfoSigned> = agents.iter().map(|x| x.1.clone()).collect();
    let alice_cert = cert_from_info(agents[0].1.clone());
    let alice = setup_empty_player(
        ShardedGossipLocalState {
            local_agents: maplit::hashset!(agents[0].0.clone()),
            ..Default::default()
        },
        agents.clone(),
    )
    .await;
    let bob = setup_empty_player(
        ShardedGossipLocalState {
            local_agents: maplit::hashset!(agents[1].0.clone()),
            ..Default::default()
        },
        agents.clone(),
    )
    .await;

    // Trying to initiate a round should succeed.
    let (tgt_cert, _, _) = alice
        .try_initiate(&mut AgentInfoSession::new(
            alice.query_agents_by_local_agents().await.unwrap(),
            all_agents.clone(),
        ))
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
    let r = alice
        .try_initiate(&mut AgentInfoSession::new(
            alice.query_agents_by_local_agents().await.unwrap(),
            all_agents.clone(),
        ))
        .await
        .unwrap();

    // Doesn't re-initiate.
    assert!(r.is_none());
    alice
        .inner
        .share_mut(|i, _| {
            assert!(i.initiate_tgt.is_some());
            Ok(())
        })
        .unwrap();

    // Wait slightly longer than the timeout.
    tokio::time::sleep(
        alice.tuning_params.gossip_round_timeout() + std::time::Duration::from_millis(1),
    )
    .await;

    let (tgt2_cert, _, alice_initiate) = alice
        .try_initiate(&mut AgentInfoSession::new(
            alice.query_agents_by_local_agents().await.unwrap(),
            all_agents.clone(),
        ))
        .await
        .unwrap()
        .expect("Failed to initiate");

    // Now it should re-initiate with a different node.
    assert_ne!(tgt_cert, tgt2_cert);
    alice
        .inner
        .share_mut(|i, _| {
            assert!(i.initiate_tgt.is_some());
            Ok(())
        })
        .unwrap();

    // Process the initiate with Bob.
    let bob_outgoing = bob
        .process_incoming(
            alice_cert.clone().into(),
            alice_initiate,
            &mut AgentInfoSession::new(
                bob.query_agents_by_local_agents().await.unwrap(),
                all_agents.clone(),
            ),
        )
        .await
        .unwrap();

    // Process the Bob's accept with Alice.
    for bo in bob_outgoing {
        alice
            .process_incoming(
                tgt2_cert.clone(),
                bo,
                &mut AgentInfoSession::new(
                    alice.query_agents_by_local_agents().await.unwrap(),
                    all_agents.clone(),
                ),
            )
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
    tokio::time::sleep(alice.tuning_params.gossip_round_timeout() / 2).await;

    // Get the map so the round doesn't timeout
    alice
        .inner
        .share_mut(|i, _| {
            i.round_map.get(&tgt2_cert);
            Ok(())
        })
        .unwrap();

    tokio::time::sleep(
        alice.tuning_params.gossip_round_timeout() / 2 + std::time::Duration::from_millis(1),
    )
    .await;

    // Check that initiating again doesn't do anything.

    let r = alice
        .try_initiate(&mut AgentInfoSession::new(
            alice.query_agents_by_local_agents().await.unwrap(),
            all_agents.clone(),
        ))
        .await
        .unwrap();
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
