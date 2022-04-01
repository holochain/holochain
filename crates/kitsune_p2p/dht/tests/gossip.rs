#![cfg(feature = "test_utils")]

use std::collections::HashMap;

use kitsune_p2p_dht::{
    arq::*,
    hash::AgentKey,
    op::*,
    persistence::*,
    region::*,
    spacetime::*,
    test_utils::{
        generate_ideal_coverage, gossip_direct, gossip_direct_at, seeded_rng, OpData, TestNode,
    },
};
use kitsune_p2p_timestamp::Timestamp;
use maplit::hashmap;
use rand::Rng;

#[test]
fn test_basic() {
    let topo = Topology::unit_zero();
    let gopa = GossipParams::new(1.into(), 0);
    let ts = |t: u32| TimeQuantum::from(t).to_timestamp_bounds(&topo).0;

    let alice_arq = Arq::new(8, (-128i32 as u32).into(), 4.into());
    let bobbo_arq = Arq::new(8, 0u32.into(), 4.into());
    let (mut alice, _) = TestNode::new_single(topo.clone(), gopa, alice_arq);
    let (mut bobbo, _) = TestNode::new_single(topo.clone(), gopa, bobbo_arq);

    alice.integrate_op(OpData::fake(0.into(), ts(10), 4321));
    bobbo.integrate_op(OpData::fake(128.into(), ts(20), 1234));

    let ta = TimeQuantum::from(30);
    let tb = TimeQuantum::from(31);
    let nta = TelescopingTimes::new(ta).segments().len() as u32;
    let ntb = TelescopingTimes::new(tb).segments().len() as u32;

    let stats = gossip_direct((&mut alice, ta), (&mut bobbo, tb)).unwrap();

    assert_eq!(stats.regions_sent, 3 * nta);
    assert_eq!(stats.regions_rcvd, 3 * ntb);
    assert_eq!(stats.op_data_sent, 4321);
    assert_eq!(stats.op_data_rcvd, 1234);
}

#[test]
fn test_multi() {
    todo!("tests of multiple arcs per node, with different powers.")
}

#[test]
fn gossip_scenario_full_sync() {
    observability::test_run().ok();
    let topo = Topology::standard_zero();
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

    let expected_num_space_chunks: u32 = 8;
    let expected_num_time_chunks: u32 = 22;

    let max_time = TimeQuantum::from(525600 / 12).to_timestamp_bounds(&topo).0; // 1 year
    assert_eq!(
        TelescopingTimes::new(topo.time_quantum(max_time))
            .segments()
            .len() as u32,
        expected_num_time_chunks
    );

    // these arqs will all be Full coverage
    let arqs = generate_ideal_coverage(&topo, &mut rng, &strat, None, n as u32, 0.0);

    let mut nodes: Vec<_> = arqs
        .iter()
        .map(|a| {
            assert_eq!(a.count(), expected_num_space_chunks);
            TestNode::new_single(topo.clone(), gopa, *a).0
        })
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

    let full_region = RegionCoords {
        space: SpaceSegment::new(31, 0),
        time: TimeSegment::new(31, 0),
    };

    // Assert that each node has the expected number of ops to start with,
    // and print each arq at the same time.
    assert_eq!(
        nodes
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let ops = n.query_op_data(&full_region);
                println!("{}", n.ascii_arqs_and_ops(&topo, i, 64));
                ops.len()
            })
            .collect::<Vec<_>>(),
        vec![ops_per_node; n]
    );

    // Do a bunch of gossip such that node 0 will be exposed to all ops created
    for p in 0..pow {
        let x: usize = 2usize.pow(p + 1);
        for i in (0..n).step_by(x) {
            let a = i;
            let b = i + x / 2;
            let (n1, n2) = get_two_mut(nodes.as_mut_slice(), a, b);
            let stats = gossip_direct_at(n1, n2, topo.time_quantum(max_time)).unwrap();

            // Something is wrong if we're sending tons of regions
            assert_eq!(
                stats.regions_sent,
                expected_num_space_chunks * expected_num_time_chunks
            );
            assert_eq!(
                stats.regions_rcvd,
                expected_num_space_chunks * expected_num_time_chunks
            );
            println!(
                "{:>2} <-> {:<2}  regions sent/rcvd: {}/{}, ops sent/rcvd: {:3}/{:3}, bytes sent/rcvd: {}/{}",
                a, b, stats.regions_sent, stats.regions_rcvd, stats.ops_sent, stats.ops_rcvd, stats.op_data_sent, stats.op_data_rcvd
            );
        }
    }

    for (i, n) in nodes.iter().enumerate() {
        println!("{}", n.ascii_arqs_and_ops(&topo, i, 64));
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
