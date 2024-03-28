//! Tests of arq resizing behavior.

#![cfg(feature = "test_utils")]

mod common;

use kitsune_p2p_dht::arq::print_arq;
use kitsune_p2p_dht::prelude::print_arqs;
use kitsune_p2p_dht::prelude::ArqClamping;
use kitsune_p2p_dht::spacetime::Topology;
use kitsune_p2p_dht::*;

use kitsune_p2p_dht::test_utils::generate_ideal_coverage;
use kitsune_p2p_dht::test_utils::seeded_rng;
use kitsune_p2p_dht_arc::DhtArcRange;

fn resize_to_equilibrium(view: &PeerViewQ, arq: &mut Arq) {
    while view.update_arq(arq) {}
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
        ..ArqStrat::default()
    };
    let jitter = 0.01;

    // generate peers with a bit too much coverage (14 > 12)
    let peers: Vec<_> = generate_ideal_coverage(&topo, &mut rng, &strat, Some(14.5), 100, jitter);
    let peer_power = peers.iter().map(|p| p.power()).min().unwrap();
    let view = PeerViewQ::new(topo.clone(), strat.clone(), peers);

    // start with a full arq at max power
    let mut arq = Arq::new_full(&topo, 0u32.into(), topo.space.max_power(&strat));
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
        ..ArqStrat::default()
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
        let stats = view.update_arq_with_stats(&mut arq);
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
        ..ArqStrat::default()
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
    while view.update_arq(&mut arq) {
        print_arq(&topo, &arq, 64);
    }
    // ensure that the arq grows to full size
    assert_eq!(arq.power(), topo.space.max_power(&strat));
    assert_eq!(arq.count(), 8);
    assert!(arq::is_full(&topo, arq.power(), arq.count()));
}

#[test]
/// Test that even if half of the nodes are clamped to an empty arc, the overall DHT
/// achieves its coverage target.
fn test_clamp_empty() {
    let topo = Topology::unit_zero();
    let mut rng = seeded_rng(None);

    let cov = 30.0;
    let strat = ArqStrat {
        min_coverage: cov,
        buffer: 0.2,
        max_power_diff: 2,
        ..ArqStrat::default()
    };
    let jitter = 0.0;
    dbg!(strat.max_chunks());

    let mut strat_clamped = strat.clone();
    strat_clamped.local_storage.arc_clamping = Some(ArqClamping::Full);

    let mut changed = true;
    let mut rounds = 0;

    // every other node is empty
    let clamp_every = 2;
    let do_clamp = |i| i % clamp_every == 0;

    // Generate all peers starting with the size they would have if all nodes were honest,
    // but then clamp every other arc to 0.
    // If none of the arcs were 0, then no arcs would grow, but in the presence of these zero arcs,
    // the honest arcs grow.
    // NOTE: this is a precursor to what we actually want, which is for nodes to detect whether other nodes
    // are slacking. In order to do that, we need to gather several observations of them over time, which we
    // don't currently do. We need observations over time, because a slacker is a node whose arc is not only
    // smaller than expected, but also is not growing. If we don't take the rate of change into account, the
    // system oscillates unstably.
    // However, we can safely assume that any node with a zero arc has chosen that intentionally, with no
    // plans of growing. (If they do grow, then they will be included in arc calculations on the next round.)
    let mut peers: Vec<_> = generate_ideal_coverage(&topo, &mut rng, &strat, None, 100, jitter)
        .into_iter()
        .enumerate()
        .map(|(i, a)| {
            if do_clamp(i) {
                Arq::empty(&topo, 10).to_arq(&topo, |_| a.start)
            } else {
                a
            }
        })
        .collect();
    let num_peers = peers.len();
    dbg!(num_peers);

    while changed {
        let view = PeerViewQ::new(topo.clone(), strat.clone(), peers.clone());
        changed = false;
        for (i, mut arq) in peers.iter_mut().enumerate() {
            if do_clamp(i) {
                // *arq = Arq::new_full(&topo, arq.start, topo.space.max_power(&strat));
                *arq.count_mut() = 0;
            } else {
                let stats = view.update_arq_with_stats(&mut arq);
                if stats.changed {
                    changed = true;
                }
            }
        }
        rounds += 1;
    }
    print_arqs(&topo, &peers, 64);
    dbg!(rounds);

    let view_unclamped = PeerViewQ::new(
        topo.clone(),
        strat.clone(),
        peers
            .clone()
            .into_iter()
            .enumerate()
            .filter_map(|(i, a)| (!do_clamp(i)).then_some(a))
            .collect(),
    );
    let view_full = PeerViewQ::new(topo.clone(), strat.clone(), peers);
    dbg!(view_unclamped.actual_coverage());
    dbg!(view_full.actual_coverage());

    // assert!(view_unclamped.actual_coverage() * 2.0 >= strat.min_coverage);
    assert!(view_full.actual_coverage() >= strat.min_coverage);
    assert!(view_full.actual_coverage() <= strat.max_coverage());
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
        ..ArqStrat::default()
    };
    let jitter = 0.01;

    // generate peers with far too little coverage
    let peers: Vec<_> = generate_ideal_coverage(&topo, &mut rng, &strat, Some(5.0), 1000, jitter);
    let peer_power = peers.iter().map(|p| p.power()).min().unwrap();
    let view = PeerViewQ::new(topo.clone(), strat.clone(), peers);

    let arq = Arq::new(peer_power - 1, 0u32.into(), 6.into());
    let mut resized = arq.clone();
    view.update_arq(&mut resized);
    assert!(resized.power() > arq.power() || resized.count() > arq.count() + 1);
}

#[test]
/// If the space to our left is oversaturated by double,
/// and the space to our right is completely empty,
/// we should not resize
///
/// (not a very good test, probably)
fn test_degenerate_asymmetrical_coverage() {
    holochain_trace::test_run().ok();
    let topo = Topology::unit_zero();
    let other = ArqBounds::from_interval(&topo, 4, DhtArcRange::from_bounds(0x0u32, 0x80))
        .unwrap()
        .to_arq(&topo, |l| l);
    let others = vec![other; 10];
    // aim for coverage between 5 and 6.
    let strat = ArqStrat {
        min_coverage: 5.0,
        buffer: 0.1,
        ..ArqStrat::default()
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
    let resized = view.update_arq(&mut new);
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
        ..ArqStrat::default()
    };
    let jitter = 0.000;

    {
        // start with a full arq
        let mut arq = Arq::new_full(&topo, Loc::new(0x0), topo.space.max_power(&strat));
        // create 10 peers, all with full arcs, fully covering the DHT
        let peers: Vec<_> = generate_ideal_coverage(&topo, &mut rng, &strat, None, 10, jitter);
        let view = PeerViewQ::new(topo.clone(), strat.clone(), peers);
        let extrapolated = view.extrapolated_coverage(&arq);
        assert_eq!(extrapolated, 10.0);

        // expect that the arq remains full under these conditions
        let resized = view.update_arq(&mut arq);
        assert!(!resized);
    }

    {
        // start with a full arq again
        let mut arq = Arq::new_full(&topo, Loc::new(0x0), topo.space.max_power(&strat));
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
            while view.update_arq(&mut arq) {}

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
            while view.update_arq(&mut arq) {
                print_arq(&topo, &arq, 64);
            }
            assert_eq!(arq.power(), peer_power + strat.max_power_diff);
            assert!(arq.count() == strat.max_chunks());
        }
    }
}
