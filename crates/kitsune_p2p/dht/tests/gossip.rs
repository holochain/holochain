use kitsune_p2p_dht::{coords::Topology, op::OpData, test_utils::test_node::TestNode};
use kitsune_p2p_timestamp::Timestamp;

/*
Integrate ops into the OpStore and the SpacetimeTree
Test generating the fingerprint, based on data in the tree
*/

#[test]
fn test_basic() {
    let topo = Topology::identity(Timestamp::from_micros(0));
    let alice = TestNode::new(topo.clone(), todo!());
    let bob = TestNode::new(topo.clone(), todo!());

    OpData::fake(0, 0, 10);
}
