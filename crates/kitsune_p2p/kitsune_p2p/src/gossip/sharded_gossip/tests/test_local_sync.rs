use super::common::*;
use super::handler_builder::{calculate_missing_ops, mock_agent_persistence, HandlerBuilder};
use super::*;
use crate::test_util::scenario_def_local::*;
use crate::NOISE;

/// Defines a sharded scenario where:
/// - There are 3 agents and 6 distinct ops between them.
/// - Each agent has an arc that covers 3 of the ops.
/// - The start of each arc overlaps with the end of one other arc,
///     so that all 3 arcs cover the entire space
/// - Each agent holds an op at the start of their arc, as well as one in the middle,
///     but is missing the one at the end of their arc.
///
/// When syncing, we expect the missing op at the end of each arc to be received
/// from the agent whose arc start intersects our arc end.
pub(super) fn three_way_sharded_ownership() -> (Vec<Arc<KitsuneAgent>>, LocalScenarioDef) {
    let agents = agents(3);
    let alice = agents[0].clone();
    let bobbo = agents[1].clone();
    let carol = agents[2].clone();
    let ownership = vec![
        // NB: each agent has an arc that covers 3 ops, but the op at the endpoint
        //     of the arc is intentionally missing
        (alice.clone(), (5, 1), vec![5, 0]),
        (bobbo.clone(), (1, 3), vec![1, 2]),
        (carol.clone(), (3, 5), vec![3, 4]),
    ];
    (agents, LocalScenarioDef::from_compact(6, ownership))
}

#[tokio::test(flavor = "multi_thread")]
async fn local_sync_scenario() {
    observability::test_run().ok();
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let space = Arc::new(KitsuneSpace::arbitrary(&mut u).unwrap());
    let (agents, ownership) = three_way_sharded_ownership();
    let (data, _) = mock_agent_persistence(&mut u, ownership);
    let agent_arcs: Vec<_> = data.iter().map(|(info, _)| info.to_agent_arc()).collect();
    let delta = calculate_missing_ops(&data);
    let delta_counts = delta.iter().map(|(_, hs)| hs.len()).collect::<Vec<_>>();

    let agent_arc_map: HashMap<_, _> = agent_arcs.clone().into_iter().collect();
    for (_, arc) in agent_arcs.iter() {
        println!("arc: |{}|", arc.to_ascii(64));
    }

    // - The test is set up so each agent is missing 1 op
    assert_eq!(delta_counts, vec![1, 1, 1]);

    let mut evt_handler = HandlerBuilder::new().with_agent_persistence(data).build();

    // Set up expectations to ensure that the proper data is gossiped to each agent,
    // while still also allowing flexibility for some extraneous gossip
    for (agent, hashes) in delta {
        let agent = agent.clone();
        let hashes: HashSet<_> = hashes.iter().cloned().collect();
        // - Ensure that the agents with missing ops get gossiped those ops
        let agent = agent.clone();
        evt_handler
            .expect_handle_gossip()
            .times(1)
            .withf(move |_, to_agent, ops| {
                *to_agent == agent && ops.iter().all(|op| hashes.contains(&*op.0))
            })
            .returning(move |_, _, _| unit_ok_fut());
    }

    // - It's OK if other agents who already hold this hash get it gossiped again,
    //     (in this case, one other agent already properly holds it)
    //     but we *don't* want agents with arcs not covering this hash to receive it
    evt_handler
        .expect_handle_gossip()
        .times(0..6)
        .withf(move |_, to_agent, ops| {
            let arc = agent_arc_map.get(to_agent).unwrap();
            ops.iter().all(|op| arc.contains(op.0.get_loc()))
        })
        .returning(move |_, _, _| unit_ok_fut());

    let (evt_sender, task) = spawn_handler(evt_handler).await;
    let gossip = ShardedGossipLocal::test(
        GossipType::Recent,
        evt_sender.clone(),
        ShardedGossipLocalState {
            local_agents: agents.clone().into_iter().collect(),
            ..Default::default()
        },
    );

    // Ensure that before local sync, a single agent only holds 2 ops
    {
        let (hashes_before, _) = store::all_op_hashes_within_arcset(
            &evt_sender,
            &space,
            // Only look at the first agent
            &agent_arcs[0..1],
            &DhtArcSet::Full,
            full_time_window(),
            usize::MAX,
            false,
        )
        .await
        .unwrap()
        .unwrap();

        let ops = store::fetch_ops(&evt_sender, &space, agents.iter().take(1), hashes_before)
            .await
            .unwrap();

        assert_eq!(ops.len(), 2);
    }

    // Run gossip, and let the handle_gossip expectations on the mock handler
    // test that the correct ops went to the correct agents
    gossip.local_sync().await.unwrap();

    // We can't actually test that agents hold the extra ops after sync, because
    // we're using an immutable mock, but by testing that handle_gossip is called
    // the appropriate amount, we ensure that in a real situation, sync is achieved.
    //
    // maackle: this kind of sucks. We have to await the handler task in order
    //          to join any panics from that task into the test to cause a failure.
    //          However, if there were no panics, then the task will never end,
    //          so we need to wait "long enough" for any panics to occur
    match tokio::time::timeout(tokio::time::Duration::from_secs(3), task).await {
        // Err means timeout, which means no panic, which means the test passed
        Err(_) => (),
        // Ok means the task ended, which means there was a panic, which means the test should fail
        Ok(err) => panic!("handler task panicked: {:?}", err),
    }
}
