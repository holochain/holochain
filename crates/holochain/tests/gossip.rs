#![cfg(feature = "no-hash-integrity")]

use std::collections::HashSet;

use holochain::sweettest::*;
use holochain_p2p::dht_arc::*;
use maplit::hashset;

async fn get_locations(conductor: &SweetConductor, apps: &SweetAppBatch) -> HashSet<i32> {
    // TODO: write conductor.get_all_op_locations in terms of IntoIterator<Item = SweetCell>
    conductor
        .get_all_op_hashes(apps.cells_flattened())
        .await
        .map(|h| h.get_loc().to_u32() as i32)
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_1() {
    use scenario::*;

    let nodes = [
        Node::new([
            Agent::new(ArcInterval::new(-30, 30), [-10, 10]),
            Agent::new(ArcInterval::new(0, 60), [10, 20]),
        ]),
        Node::new([
            Agent::new(ArcInterval::new(-40, 40), [-40, -20, 20, 40]),
            Agent::new(ArcInterval::new(-60, 0), [-60, -30]),
        ]),
        Node::new([Agent::new(ArcInterval::new(-120, -60), [] as [i32; 0])]),
        // Node::new([Agent::new(ArcInterval::new(-120, -60), [-90, -60])]),
    ];
    // let scenario = ScenarioDef::new(nodes, PeerMatrix::sparse([&[], &[], &[]]));
    let scenario = ScenarioDef::new(nodes, PeerMatrix::sparse([&[1, 2], &[0, 2], &[]]));
    let [(c0, a0), (c1, a1), (c2, a2)] =
        SweetConductorBatch::setup_from_scenario(scenario, unit_dna().await).await;

    let locs0 = get_locations(&c0, &a0).await;
    let locs1 = get_locations(&c1, &a1).await;
    let locs2 = get_locations(&c2, &a2).await;

    dbg!((locs0.len(), locs1.len(), locs2.len()));
    dbg!(&locs0);
    dbg!(&locs1);
    dbg!(&locs2);

    // TODO: properly await consistency
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    dbg!((locs0.len(), locs1.len(), locs2.len()));
    let locs0 = get_locations(&c0, &a0).await;
    let locs1 = get_locations(&c1, &a1).await;
    let locs2 = get_locations(&c2, &a2).await;

    dbg!(&locs0);
    dbg!(&locs1);
    dbg!(&locs2);

    assert_eq!((locs0.len(), locs1.len(), locs2.len()), (99, 99, 99));
    assert_eq!(locs0, hashset![-60, -40, -30, -20, -10, 10, 20, 40]);
}
