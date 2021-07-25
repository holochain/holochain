use maplit::hashset;

use super::common::*;
use super::handler_builder::{generate_ops_for_overlapping_arcs, handler_builder, OwnershipData};
use super::*;

fn three_way_sharded_ownership() -> (HashSet<Arc<KitsuneAgent>>, OwnershipData<6>) {
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
async fn test_three_way_sharded_ownership() {
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let space = Arc::new(KitsuneSpace::arbitrary(&mut u).unwrap());
    let (agents, ownership) = three_way_sharded_ownership();
    let data = generate_ops_for_overlapping_arcs(&mut u, ownership);
    let agent_arcs: Vec<_> = data
        .iter()
        .map(|(agent, arc, _)| (agent.clone(), arc.clone()))
        .collect();

    let mut evt_handler = handler_builder(data).await;
    let (evt_sender, _) = spawn_handler(evt_handler).await;

    let get_op_hashes = |a: usize| async move {
        store::all_op_hashes_within_arcset(
            &evt_sender,
            &space,
            // Only look at one agent at a time
            &agent_arcs[a..a + 1],
            &DhtArcSet::Full,
            full_time_window(),
            usize::MAX,
        )
        .await
        .unwrap()
        .unwrap()
        .0
    };

    let op_hashes_0 = (get_op_hashes.clone())(0).await;
    let op_hashes_1 = (get_op_hashes.clone())(1).await;
    let op_hashes_2 = (get_op_hashes.clone())(2).await;
    assert_eq!(
        (op_hashes_0.len(), op_hashes_1.len(), op_hashes_2.len()),
        (2, 2, 2)
    );

    // let ops = store::fetch_ops(&evt_sender, &space, &agents, op_hashes_0)
    //     .await
    //     .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn local_sync_scenario() {
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let (agents, ownership) = three_way_sharded_ownership();
    let data = generate_ops_for_overlapping_arcs(&mut u, ownership);
    let mut evt_handler = handler_builder(data).await;

    let (evt_sender, _) = spawn_handler(evt_handler).await;
    let gossip = ShardedGossipLocal::test(GossipType::Recent, evt_sender, Default::default());

    // store::fetch_ops(&evt_sender, space, agents, op_hashes);

    gossip.local_sync().await.unwrap();

    let _cert = Tx2Cert::arbitrary(&mut u);
}
