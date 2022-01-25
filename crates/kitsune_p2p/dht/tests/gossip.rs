#![cfg(feature = "testing")]

use kitsune_p2p_dht::{
    arq::*,
    coords::*,
    host::*,
    op::*,
    region::*,
    test_utils::{
        generate_ideal_coverage,
        gossip_direct::{gossip_direct, gossip_direct_at},
        seeded_rng,
        test_node::TestNode,
    },
};
use kitsune_p2p_timestamp::Timestamp;
use rand::Rng;

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
    observability::test_run().ok();
    let topo = Topology::standard(Timestamp::from_micros(0));
    let gopa = GossipParams::new(1.into(), 0);
    let ts = |t: u32| topo.timestamp(t.into());

    let mut rng = seeded_rng(None);

    let strat = ArqStrat {
        min_coverage: 5.0,
        ..Default::default()
    };

    // must be a power of 2.
    let pow = 4;
    let n = 2usize.pow(pow); // 128;
    let ops_per_node = 10;

    let max_time = topo.timestamp(TimeCoord::from(525600 / 12)); // 1 year

    let arqs = generate_ideal_coverage(&mut rng, &strat, None, n as u32, 0.0, 0);
    for (i, arq) in arqs.iter().enumerate() {
        println!(
            "|{}| {}: {}/{} @ {}",
            arq.to_ascii(64),
            i,
            arq.power(),
            arq.count(),
            arq.center()
        );
    }
    let mut nodes: Vec<_> = arqs
        .iter()
        .map(|a| TestNode::new(topo.clone(), gopa, *a))
        .collect();

    let ops = std::iter::repeat_with(|| {
        OpData::fake(
            Loc::new(rng.gen()),
            Timestamp::from_micros(rng.gen_range(0..max_time.as_micros())),
            rng.gen_range(1..16_000_000),
        )
    })
    .take(n as usize * ops_per_node);

    for (i, op) in ops.enumerate() {
        nodes[i % n].integrate_op(op);
    }

    let full_region = RegionBounds {
        x: (0.into(), u32::MAX.into()),
        t: (0.into(), u32::MAX.into()),
    };
    assert_eq!(
        nodes
            .iter()
            .map(|n| n.query_op_data(&full_region).len())
            .collect::<Vec<_>>(),
        vec![ops_per_node; n]
    );

    for p in 1..pow {
        let x: usize = 2usize.pow(p);
        for i in (0..n / x).step_by(x) {
            let a = i;
            let b = i + x / 2;
            dbg!("{}, {}", a, b);
            let (n1, n2) = get_two_mut(nodes.as_mut_slice(), a, b);
            let stats = gossip_direct_at(n1, n2, topo.time_coord(max_time)).unwrap();
            dbg!(stats);
        }
    }

    assert_eq!(
        nodes
            .iter()
            .map(|n| n.query_op_data(&full_region).len())
            .collect::<Vec<_>>(),
        vec![ops_per_node * n; n]
    )
}

/// from https://www.reddit.com/r/rust/comments/7dep46/multiple_references_to_a_vectors_elements/
fn get_two_mut<T>(slice: &mut [T], index1: usize, index2: usize) -> (&mut T, &mut T) {
    assert!(index1 != index2 && index1 < slice.len() && index2 < slice.len());
    if index1 < index2 {
        let (start, end) = slice.split_at_mut(index2);
        (&mut start[index1], &mut end[0])
    } else {
        let (start, end) = slice.split_at_mut(index1);
        (&mut end[0], &mut start[index2])
    }
}
