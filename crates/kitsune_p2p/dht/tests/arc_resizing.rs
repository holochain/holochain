//! Tests of arq resizing behavior.

mod common;
use common::*;

use kitsune_p2p_dht::arq::*;
use kitsune_p2p_dht::op::*;
use kitsune_p2p_dht_arc::ArcInterval;

use crate::common::quantized::generate_ideal_coverage;
use crate::common::quantized::print_arqs;
use crate::common::quantized::seeded_rng;

#[test]
/// If extrapolated coverage remains above the maximum coverage threshold even
/// when shrinking to empty, let the arq be resized to empty.
fn test_shrink_to_empty() {
    todo!()
}

#[test]
/// If extrapolated coverage remains below the minimum coverage threshold even
/// when growing to full, let the arq be resized to full.
fn test_grow_to_full() {
    todo!()
}

#[test]
/// If the current coverage is far from the target, shrinking can occur in
/// multiple chunks
fn test_shrink_by_multiple_chunks() {
    todo!()
}

#[test]
/// If the current coverage is far from the target, growing can occur in
/// multiple chunks
fn test_grow_by_multiple_chunks() {
    todo!()
}

#[test]
/// If the space to our left is completely oversaturated
/// and the space to our right is completely undersaturated,
/// we should resize our arq so that the average coverage is within the
/// acceptable range
fn test_degenerate_asymmetrical_coverage() {
    let a = Arq::new(
        Loc::from(0x80),
        4, // 0x10
        0x10,
    );
    assert_eq!(a.to_interval(), ArcInterval::new(0, 0x100 - 1));

    let other = ArqBounds::from_interval(4, ArcInterval::new(0x0, 0x80)).unwrap();
    let others = ArqSet::new(vec![other; 20]);
    // aim for coverage between 5 and 6.
    let strat = ArqStrat {
        min_coverage: 5.0,
        buffer: 0.2,
        ..Default::default()
    };
    let view = PeerView::new(strat, others);
    let extrapolated = view.extrapolated_coverage(&a.to_bounds());
    assert_eq!(extrapolated, 10.0);
    let resized = view.update_arq(a);
}

#[test]
/// Test resizing across several of quantization levels to get a feel for how
/// it should work.
fn test_scenario() {
    let mut rng = seeded_rng(None);

    // aim for coverage between 10 and 12.
    let strat = ArqStrat {
        min_coverage: 10.0,
        buffer: 0.2,
        ..Default::default()
    };
    let jitter = 0.000;

    {
        // start with a full arq
        let arq = Arq::new_full(Loc::from(0x0), strat.max_power);
        let peers: Vec<_> = generate_ideal_coverage(&mut rng, &strat, 10, jitter, 0)
            .into_iter()
            .map(|arq| arq.to_bounds())
            .collect();
        let view = PeerView::new(strat.clone(), ArqSet::new(peers));
        let extrapolated = view.extrapolated_coverage(&arq.to_bounds());
        assert_eq!(extrapolated, 11.0);

        // expect that the arq remains full under these conditions
        let resized = view.update_arq(arq.clone());
        assert_eq!(resized, arq);
    }

    {
        // start with a full arq again
        let arq = Arq::new_full(Loc::from(0x0), strat.max_power);
        let peer_arqs = generate_ideal_coverage(&mut rng, &strat, 100, jitter, 0);

        let peers = ArqSet::new(peer_arqs.into_iter().map(|arq| arq.to_bounds()).collect());
        print_arqs(&peers, 64);

        let view = PeerView::new(strat.clone(), peers);
        let extrapolated = view.extrapolated_coverage(&arq.to_bounds());
        assert_eq!(extrapolated, 10.0);

        // expect that the arq shrinks
        let resized = view.update_arq(arq.clone());
        assert_eq!(resized.power(), strat.max_power);
        assert_eq!(resized.count(), 8);
    }

    todo!("add more peers and watch it upsample and shrink")
}
