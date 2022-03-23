//! Tests of arq resizing behavior.

#![cfg(feature = "test_utils")]

mod common;

use kitsune_p2p_dht::arq::print_arq;
use kitsune_p2p_dht::quantum::Topology;
use kitsune_p2p_dht::*;

use kitsune_p2p_dht::test_utils::generate_ideal_coverage;
use kitsune_p2p_dht::test_utils::seeded_rng;
use kitsune_p2p_dht_arc::DhtArcRange;

fn resize_to_equilibrium(view: &PeerViewQ, arq: &mut Arq) {
    while view.update_arq(&view.topo, arq) {}
}

#[test]
/// If extrapolated coverage remains above the maximum coverage threshold even
/// when shrinking towards empty, let the arq be resized as small as possible
/// before losing peers.
fn test_shrink_towards_empty() {
    let topo = Topology::unit_zero();
    let mut rng = seeded_rng(None);

    // aim for coverage between 10 and 12
    let strat = ArqStrat {
        min_coverage: 10.0,
        buffer: 0.2,
        max_power_diff: 2,
        ..Default::default()
    };
    let jitter = 0.01;

    // generate peers with a bit too much coverage (14 > 12)
    let peers: Vec<_> = generate_ideal_coverage(&topo, &mut rng, &strat, Some(14.5), 100, jitter);
    let peer_power = peers.iter().map(|p| p.power()).min().unwrap();
    let view = PeerViewQ::new(topo.clone(), strat.clone(), peers);

    // start with a full arq at max power
    let mut arq = Arq::new_full(&topo, 0u32.into(), topo.max_space_power(&strat));
    resize_to_equilibrium(&view, &mut arq);
    // test that the arc gets reduced in power to match those of its peers
    assert!(
        arq.power() <= peer_power,
        "{} <= {}",
        arq.power(),
        peer_power
    );
}

#[test]
/// If extrapolated coverage remains below the minimum coverage threshold even
/// when growing to full, let the arq be resized as large as it can be under
/// the constraints of the ArqStrat.
fn test_grow_towards_full() {
    let topo = Topology::unit_zero();
    let mut rng = seeded_rng(None);

    // aim for coverage between 10 and 12, with no limit on power diff
    let strat = ArqStrat {
        min_coverage: 10.0,
        buffer: 0.2,
        max_power_diff: 2,
        ..Default::default()
    };
    println!("{}", strat.summary());
    strat.max_chunks();
    let jitter = 0.01;

    // generate peers with deficient coverage
    let peers: Vec<_> = generate_ideal_coverage(&topo, &mut rng, &strat, Some(7.0), 1000, jitter);
    let peer_power = peers.iter().map(|p| p.power()).min().unwrap();
    let view = PeerViewQ::new(topo.clone(), strat.clone(), peers);

    // start with an arq comparable to one's peers
    let mut arq = Arq::new(peer_power, 0u32.into(), 12.into());
    loop {
        let stats = view.update_arq_with_stats(&topo, &mut arq);
        if !stats.changed {
            break;
        }
    }
    // ensure that the arq grows to full size
    assert_eq!(arq.power(), peer_power + 2);
    assert_eq!(arq.count(), strat.max_chunks());
}

#[test]
/// If extrapolated coverage remains below the minimum coverage threshold even
/// when growing to full, let the arq be resized to full when the max_power_diff
/// is not a constraint
fn test_grow_to_full() {
    let topo = Topology::unit_zero();
    let mut rng = seeded_rng(None);

    // aim for coverage between 10 and 12, with no limit on power diff
    let strat = ArqStrat {
        min_coverage: 10.0,
        buffer: 0.2,
        max_power_diff: 32,
        ..Default::default()
    };
    let jitter = 0.01;
    dbg!(strat.max_chunks());

    // generate peers with deficient coverage
    let peers: Vec<_> = generate_ideal_coverage(&topo, &mut rng, &strat, Some(7.0), 1000, jitter);
    let peer_power = peers.iter().map(|p| p.power()).min().unwrap();
    let view = PeerViewQ::new(topo.clone(), strat.clone(), peers);

    // start with an arq comparable to one's peers
    let mut arq = Arq::new(peer_power, 0u32.into(), 12.into());
    print_arq(&topo, &arq, 64);
    while view.update_arq(&topo, &mut arq) {
        print_arq(&topo, &arq, 64);
    }
    // ensure that the arq grows to full size
    assert_eq!(arq.power(), topo.max_space_power(&strat));
    assert_eq!(arq.count(), 8);
    assert!(arq::is_full(&topo, arq.power(), arq.count()));
}

#[test]
// XXX: We only want to do this if other peers have not moved. But currently
//      we have no way of determining this.
//
/// If the current coverage is far from the target, growing can occur in
/// multiple chunks
fn test_grow_by_multiple_chunks() {
    let topo = Topology::unit_zero();
    let mut rng = seeded_rng(None);

    // aim for coverage between 10 and 12
    let strat = ArqStrat {
        min_coverage: 10.0,
        buffer: 0.2,
        ..Default::default()
    };
    let jitter = 0.01;

    // generate peers with far too little coverage
    let peers: Vec<_> = generate_ideal_coverage(&topo, &mut rng, &strat, Some(5.0), 1000, jitter);
    let peer_power = peers.iter().map(|p| p.power()).min().unwrap();
    let view = PeerViewQ::new(topo.clone(), strat.clone(), peers);

    let arq = Arq::new(peer_power - 1, 0u32.into(), 6.into());
    let mut resized = arq.clone();
    view.update_arq(&topo, &mut resized);
    assert!(resized.power() > arq.power() || resized.count() > arq.count() + 1);
}

#[test]
/// If the space to our left is oversaturated by double,
/// and the space to our right is completely empty,
/// we should not resize
///
/// (not a very good test, probably)
fn test_degenerate_asymmetrical_coverage() {
    observability::test_run().ok();
    let topo = Topology::unit_zero();
    let other = ArqBounds::from_interval(&topo, 4, DhtArcRange::from_bounds(0x0u32, 0x80))
        .unwrap()
        .to_arq(&topo, |l| l);
    let others = vec![other; 10];
    // aim for coverage between 5 and 6.
    let strat = ArqStrat {
        min_coverage: 5.0,
        buffer: 0.1,
        ..Default::default()
    };
    let view = PeerViewQ::new(topo.clone(), strat, others);

    let arq = Arq::new(
        4, // log2 of 0x10
        Loc::new(0),
        0x10.into(),
    );

    let extrapolated = view.extrapolated_coverage(&arq);
    assert_eq!(extrapolated, 5.0);
    let old = arq.clone();
    let mut new = arq.clone();
    let resized = view.update_arq(&topo, &mut new);
    assert_eq!(old, new);
    assert!(!resized);
}

#[test]
/// Test resizing across several quantization levels to get a feel for how
/// it should work.
fn test_scenario() {
    let mut rng = seeded_rng(None);
    let topo = Topology::unit_zero();

    // aim for coverage between 10 and 12.
    let strat = ArqStrat {
        min_coverage: 10.0,
        buffer: 0.2,
        max_power_diff: 2,
        ..Default::default()
    };
    let jitter = 0.000;

    {
        // start with a full arq
        let mut arq = Arq::new_full(&topo, Loc::new(0x0), topo.max_space_power(&strat));
        // create 10 peers, all with full arcs, fully covering the DHT
        let peers: Vec<_> = generate_ideal_coverage(&topo, &mut rng, &strat, None, 10, jitter);
        let view = PeerViewQ::new(topo.clone(), strat.clone(), peers);
        let extrapolated = view.extrapolated_coverage(&arq);
        assert_eq!(extrapolated, 10.0);

        // expect that the arq remains full under these conditions
        let resized = view.update_arq(&topo, &mut arq);
        assert!(!resized);
    }

    {
        // start with a full arq again
        let mut arq = Arq::new_full(&topo, Loc::new(0x0), topo.max_space_power(&strat));
        // create 100 peers, with arcs at about 10%,
        // covering a bit more than they need to
        let peers = generate_ideal_coverage(&topo, &mut rng, &strat, Some(13.0), 100, jitter);

        {
            let peer_power = peers.iter().map(|p| p.power()).min().unwrap();
            assert_eq!(peer_power, 26);

            let view = PeerViewQ::new(topo.clone(), strat.clone(), peers.clone());
            let extrapolated = view.extrapolated_coverage(&arq);
            assert!(extrapolated > strat.max_coverage());
            // assert!(strat.min_coverage <= extrapolated && extrapolated <= strat.max_coverage());

            // update the arq until there is no change
            while view.update_arq(&topo, &mut arq) {}

            // expect that the arq shrinks to at least the ballpark of the peers
            assert_eq!(arq.power(), peer_power);
        }
        {
            // create the same view but with all arcs cut in half, so that the
            // coverage is uniformly undersaturated.
            let peers: Vec<_> = peers
                .clone()
                .iter_mut()
                .map(|arq| {
                    let mut arq = arq.downshift();
                    *arq.count_mut() = arq.count() / 2;
                    arq
                })
                .collect();
            let peer_power = peers.iter().map(|p| p.power()).min().unwrap();
            let view = PeerViewQ::new(topo.clone(), strat.clone(), peers);
            print_arq(&topo, &arq, 64);
            // assert that our arc will grow as large as it can to pick up the slack.
            while view.update_arq(&topo, &mut arq) {
                print_arq(&topo, &arq, 64);
            }
            assert_eq!(arq.power(), peer_power + strat.max_power_diff);
            assert!(arq.count() == strat.max_chunks());
        }
    }
}
