use futures::future;
use maplit::hashset;

use super::common::*;
use super::handler_builder::{
    calculate_missing_ops, generate_ops_for_overlapping_arcs, HandlerBuilder, OwnershipData,
};
use super::*;

pub(super) fn three_way_sharded_ownership() -> (HashSet<Arc<KitsuneAgent>>, OwnershipData<6>) {
    let agents = agents(3);
    let alice = agents[0].clone();
    let bobbo = agents[1].clone();
    let carol = agents[2].clone();
    let ownership = [
        hashset![alice.clone()],
        hashset![alice.clone(), bobbo.clone()],
        hashset![bobbo.clone()],
        hashset![bobbo.clone(), carol.clone()],
        hashset![carol.clone()],
        hashset![carol.clone(), alice.clone()],
    ];
    (hashset![alice, bobbo, carol], ownership)
}

#[tokio::test(flavor = "multi_thread")]
async fn local_sync_scenario() {
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let space = Arc::new(KitsuneSpace::arbitrary(&mut u).unwrap());
    let (agents, ownership) = three_way_sharded_ownership();
    let data = generate_ops_for_overlapping_arcs(&mut u, ownership);
    let agent_arcs: Vec<_> = data
        .iter()
        .map(|(agent, arc, _)| (agent.clone(), arc.clone()))
        .collect();
    let delta = calculate_missing_ops(&data);
    let delta_counts = delta.iter().map(|(_, hs)| hs.len()).collect::<Vec<_>>();
    assert_eq!(delta_counts, vec![0, 0, 0]);

    println!("data {:#?}", data);
    println!("delta {:#?}", delta);

    let ok_fut = move || Ok(async move { Ok(()) }.boxed().into());
    let mut evt_handler = HandlerBuilder::new().with_agent_persistence(data).build();
    let mut seq = mockall::Sequence::new();

    for (agent, hashes) in delta {
        for h in hashes {
            let agent = agent.clone();
            evt_handler
                .expect_handle_gossip()
                .times(1)
                .withf(move |_, to_agent, from_agent, hash, op| {
                    dbg!(agent.clone(), to_agent, from_agent, hash, op);
                    *hash.as_ref() == h
                })
                .returning(move |_, _, _, _, _| ok_fut());
        }
    }

    let (evt_sender, _) = spawn_handler(evt_handler).await;
    let gossip = ShardedGossipLocal::test(
        GossipType::Recent,
        evt_sender.clone(),
        ShardedGossipLocalState {
            local_agents: agents.clone(),
            ..Default::default()
        },
    );

    // Ensure that before local sync, a single agent only holds 3 ops
    {
        let (hashes_before, _) = store::all_op_hashes_within_arcset(
            &evt_sender,
            &space,
            // Only look at the first agent
            &agent_arcs[0..1],
            &DhtArcSet::Full,
            full_time_window(),
            usize::MAX,
        )
        .await
        .unwrap()
        .unwrap();

        let ops = store::fetch_ops(&evt_sender, &space, agents.iter().take(1), hashes_before)
            .await
            .unwrap();

        assert_eq!(ops.len(), 3);
    }

    // Run gossip, and let the handle_gossip expectations on the mock handler
    // test that the correct ops went to the correct agents
    gossip.local_sync().await.unwrap();
}
