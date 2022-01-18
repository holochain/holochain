#![cfg(feature = "testing")]

use kitsune_p2p_dht::{
    arq::*,
    coords::*,
    host::*,
    op::*,
    region::*,
    test_utils::{gossip_direct::gossip_direct_at, test_node::TestNode},
};
use kitsune_p2p_timestamp::Timestamp;

/*
Integrate ops into the OpStore and the SpacetimeTree
Test generating the fingerprint, based on data in the tree
*/

#[test]
fn test_basic() {
    let topo = Topology::identity(Timestamp::from_micros(0));
    let gopa = GossipParams::zero();

    let alice_arq = Arq::new(0.into(), 8, 4);
    let bobbo_arq = Arq::new(128.into(), 8, 4);
    let mut alice = TestNode::new(topo.clone(), gopa, alice_arq);
    let mut bobbo = TestNode::new(topo.clone(), gopa, bobbo_arq);

    alice.integrate_op(OpData::fake(0, 10, 4321));
    bobbo.integrate_op(OpData::fake(128, 20, 1234));

    let stats = gossip_direct_at(&mut alice, &mut bobbo, Timestamp::from_micros(30)).unwrap();

    assert_eq!(stats.region_data_sent, REGION_MASS);
    assert_eq!(stats.region_data_rcvd, REGION_MASS);
    assert_eq!(stats.op_data_sent, 4321);
    assert_eq!(stats.op_data_rcvd, 1234);
}
