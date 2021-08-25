use holochain::sweettest::*;
use holochain_p2p::dht_arc::*;
use holochain_types::prelude::*;

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
        Node::new([Agent::new(ArcInterval::new(-120, -60), [-90, -60])]),
    ];
    let scenario = ScenarioDef::new(nodes, PeerMatrix::sparse([&[1, 2], &[0, 2], &[]]));
    let [(c0, _), (c1, _), (c2, _)] =
        SweetConductorBatch::setup_from_scenario(scenario, unit_dna().await).await;
}
