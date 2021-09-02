use holochain::sweettest::*;
use maplit::hashset;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread")]
async fn test_1() {
    use scenario::*;
    // observability::test_run().ok();

    // TODO: seems that the first node only displays having [-10, 20].
    // See what ops these are and what's special about them.

    let arc = (-10, 10);
    let nodes = [
        Node::new([
            Agent::new(arc.clone(), [-1, 1]),
            Agent::new(arc.clone(), [-2, 2]),
        ]),
        Node::new([
            Agent::new(arc.clone(), [-3, 3]),
            Agent::new(arc.clone(), [-4, 4]),
        ]),
        Node::new([Agent::new(arc.clone(), [0])]),
    ];
    // let peers = PeerMatrix::sparse([&[1, 2], &[0, 2], &[]]);
    let peers = PeerMatrix::Full;
    let def = ScenarioDef::new(nodes, peers);
    let scenario = SweetGossipScenario::setup(def, unit_dna().await).await;
    let [c0, c1, c2] = scenario.nodes();

    let locs0 = c0.get_op_basis_loc_buckets().await;
    let locs1 = c1.get_op_basis_loc_buckets().await;
    let locs2 = c2.get_op_basis_loc_buckets().await;

    dbg!((locs0.len(), locs1.len(), locs2.len()));
    dbg!(&locs0);
    dbg!(&locs1);
    dbg!(&locs2);

    // TODO: properly await consistency
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let locs0 = c0.get_op_basis_loc_buckets().await;
    let locs1 = c1.get_op_basis_loc_buckets().await;
    let locs2 = c2.get_op_basis_loc_buckets().await;

    dbg!((locs0.len(), locs1.len(), locs2.len()));
    dbg!(&locs0);
    dbg!(&locs1);
    dbg!(&locs2);

    assert_eq!(
        locs0,
        hashset![-4, -3, -2, -1, 0, 1, 2, 3, 4]
    );
}
