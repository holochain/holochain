use holochain::sweettest::*;
use maplit::hashset;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread")]
async fn test_1() {
    use scenario::*;
    // observability::test_run().ok();

    // TODO: seems that the first node only displays having [-10, 20].
    // See what ops these are and what's special about them.

    let nodes = [
        Node::new([
            Agent::new((0, 60), [10, 20]),
            Agent::new((-30, 30), [-10, 10, 1, 2, 3, -1, -2, -3]),
        ]),
        Node::new([
            Agent::new((-40, 40), [-40, -20, 20, 40]),
            Agent::new((-60, 0), [-60, -30]),
        ]),
        Node::new([Agent::new((-120, -60), [-90, -60])]),
    ];
    // let peers = PeerMatrix::sparse([&[1, 2], &[0, 2], &[]]);
    let peers = PeerMatrix::Full;
    let def = ScenarioDef::new(nodes, peers);
    let scenario = SweetGossipScenario::setup(def, unit_dna().await).await;
    let [c0, c1, c2] = scenario.nodes();

    let locs0 = c0.get_op_basis_buckets().await;
    let locs1 = c1.get_op_basis_buckets().await;
    let locs2 = c2.get_op_basis_buckets().await;

    dbg!((locs0.len(), locs1.len(), locs2.len()));
    dbg!(&locs0);
    dbg!(&locs1);
    dbg!(&locs2);

    // TODO: properly await consistency
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let locs0 = c0.get_op_basis_buckets().await;
    let locs1 = c1.get_op_basis_buckets().await;
    let locs2 = c2.get_op_basis_buckets().await;

    dbg!((locs0.len(), locs1.len(), locs2.len()));
    dbg!(&locs0);
    dbg!(&locs1);
    dbg!(&locs2);

    // assert_eq!((locs0.len(), locs1.len(), locs2.len()), (99, 99, 99));
    assert_eq!(locs0, hashset![-60, -40, -30, -20, -10, 10, 20, 40]);
}
