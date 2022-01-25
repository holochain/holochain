#![cfg(feature = "testing")]

use kitsune_p2p_dht::{
    arq::ascii::add_location_ascii,
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

    assert_eq!(stats.regions_sent, 3 * nta);
    assert_eq!(stats.regions_rcvd, 3 * ntb);
    assert_eq!(stats.op_data_sent, 4321);
    assert_eq!(stats.op_data_rcvd, 1234);
}

#[test]
fn test_gossip_scenario() {
    observability::test_run().ok();
    let topo = Topology::standard(Timestamp::from_micros(0));
    let gopa = GossipParams::new(1.into(), 0);

    let mut rng = seeded_rng(None);

    // must be a power of 2.
    let pow = 4;
    let n = 2usize.pow(pow);
    let ops_per_node = 10;

    let strat = ArqStrat {
        min_coverage: n as f64,
        ..Default::default()
    };

    let max_time = topo.timestamp(TimeCoord::from(525600 / 12)); // 1 year

    let arqs = generate_ideal_coverage(&mut rng, &strat, None, n as u32, 0.0, 0);
    let mut nodes: Vec<_> = arqs
        .iter()
        .map(|a| TestNode::new(topo.clone(), gopa, *a))
        .collect();

    let num_ops = ops_per_node * n;
    let ops = std::iter::repeat_with(|| {
        OpData::fake(
            Loc::new(rng.gen()),
            Timestamp::from_micros(rng.gen_range(0..max_time.as_micros())),
            rng.gen_range(1..16_000_000),
        )
    })
    .take(num_ops);

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
            .enumerate()
            .map(|(i, n)| {
                let arq = n.arq();
                let ops = n.query_op_data(&full_region);
                println!("{}", n.ascii_arq_and_ops(i, 64));
                ops.len()
            })
            .collect::<Vec<_>>(),
        vec![ops_per_node; n]
    );

    {
        {
            let (n1, n2) = get_two_mut(nodes.as_mut_slice(), 0, 1);
            let stats = gossip_direct_at(n1, n2, topo.time_coord(max_time)).unwrap();
            assert_eq!(stats.ops_sent, ops_per_node as u32);
            assert_eq!(stats.ops_rcvd, ops_per_node as u32);
        }
        println!("vvvvvvvvvvvvvvvvvvvvvvvvvvv");
        for (i, n) in nodes.iter().take(2).enumerate() {
            println!("{}", n.ascii_arq_and_ops(i, 64));
        }
    }

    // Do a bunch of gossip such that node 0 will be exposed to all ops created
    for p in 0..pow {
        let x: usize = 2usize.pow(p + 1);
        for i in (0..n).step_by(x) {
            let a = i;
            let b = i + x / 2;
            let (n1, n2) = get_two_mut(nodes.as_mut_slice(), a, b);
            let stats = gossip_direct_at(n1, n2, topo.time_coord(max_time)).unwrap();
            println!(
                "{:>2} <-> {:<2}  regions sent/rcvd: {}/{}, ops sent/rcvd: {:3}/{:3}",
                a, b, stats.regions_sent, stats.regions_rcvd, stats.ops_sent, stats.ops_rcvd
            );
        }
    }

    for (i, n) in nodes.iter().enumerate() {
        println!("{}", n.ascii_arq_and_ops(i, 64));
    }

    assert_eq!(nodes[0].query_op_data(&full_region).len(), num_ops);
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
