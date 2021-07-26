use futures::future;
use maplit::hashset;

use super::common::*;
use super::handler_builder::{generate_ops_for_overlapping_arcs, HandlerBuilder, OwnershipData};
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

    let mut evt_handler = HandlerBuilder::new().with_agent_persistence(data).build();
    evt_handler
        .expect_handle_gossip()
        .withf(|_, to_agent, from_agent, hash, op| {
            todo!("check that this is called properly, etc")
        });
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

    gossip.local_sync().await.unwrap();

    // Ensure that after local sync, a single agent holds all 6 ops
    {
        let (hashes_after, _) = store::all_op_hashes_within_arcset(
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

        let ops = store::fetch_ops(&evt_sender, &space, agents.iter().take(1), hashes_after)
            .await
            .unwrap();

        assert_eq!(ops.len(), 6);
    }
}
