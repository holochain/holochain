#![cfg(feature = "testing")]

use kitsune_p2p_dht::{
    arq::*,
    coords::*,
    host::*,
    op::*,
    region::*,
    test_utils::{gossip_direct::gossip_direct, test_node::TestNode},
};
use kitsune_p2p_timestamp::Timestamp;

#[test]
fn test_basic() {
    let topo = Topology::identity_zero();
    let gopa = GossipParams::new(1.into(), 0);
    let ts = |t: u32| topo.timestamp(t.into());

    let alice_arq = Arq::new(0.into(), 8, 4);
    let bobbo_arq = Arq::new(128.into(), 8, 4);
    let mut alice = TestNode::new(topo.clone(), gopa, alice_arq);
    let mut bobbo = TestNode::new(topo.clone(), gopa, bobbo_arq);

    alice.integrate_op(OpData::fake(0.into(), ts(10), 4321));
    bobbo.integrate_op(OpData::fake(128.into(), ts(20), 1234));

    let ta = TimeCoord::from(30);
    let tb = TimeCoord::from(31);
    let nta = TelescopingTimes::new(ta).segments().len() as u32;
    let ntb = TelescopingTimes::new(tb).segments().len() as u32;

    let stats = gossip_direct((&mut alice, ta), (&mut bobbo, tb)).unwrap();

    assert_eq!(stats.region_data_sent, 3 * nta * REGION_MASS);
    assert_eq!(stats.region_data_rcvd, 3 * ntb * REGION_MASS);
    assert_eq!(stats.op_data_sent, 4321);
    assert_eq!(stats.op_data_rcvd, 1234);
}

#[test]
fn test_gossip_scenario() {
    let topo = Topology::standard(Timestamp::from_micros(0));
    let gopa = GossipParams::new(1.into(), 0);
    let ts = |t: u32| topo.timestamp(t.into());

    let alice_arq = Arq::new(0.into(), 8, 4);
    let bobbo_arq = Arq::new(128.into(), 8, 4);
    let mut alice = TestNode::new(topo.clone(), gopa, alice_arq);
    let mut bobbo = TestNode::new(topo.clone(), gopa, bobbo_arq);

    alice.integrate_op(OpData::fake(0.into(), ts(10), 4321));
    bobbo.integrate_op(OpData::fake(128.into(), ts(20), 1234));

    let ta = TimeCoord::from(30);
    let tb = TimeCoord::from(31);
    let nta = TelescopingTimes::new(ta).segments().len() as u32;
    let ntb = TelescopingTimes::new(tb).segments().len() as u32;

    let stats = gossip_direct((&mut alice, ta), (&mut bobbo, tb)).unwrap();

    assert_eq!(stats.region_data_sent, 3 * nta * REGION_MASS);
    assert_eq!(stats.region_data_rcvd, 3 * ntb * REGION_MASS);
    assert_eq!(stats.op_data_sent, 4321);
    assert_eq!(stats.op_data_rcvd, 1234);
}
