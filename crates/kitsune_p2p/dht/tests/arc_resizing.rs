//! Tests of arc resizing behavior.

mod common;
use common::*;

use kitsune_p2p_dht::arq::*;
use kitsune_p2p_dht::op::*;
use kitsune_p2p_dht_arc::ArcInterval;

#[test]
/// If extrapolated coverage remains above the maximum coverage threshold even
/// when shrinking to empty, let the arc be resized to empty.
fn test_shrink_to_empty() {
    todo!()
}

#[test]
/// If extrapolated coverage remains below the minimum coverage threshold even
/// when growing to full, let the arc be resized to full.
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
/// we should resize our arc so that the average coverage is within the
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
    };
    let view = PeerView::new(others);
    let extrapolated = view.extrapolated_coverage(&a.to_bounds());
    assert_eq!(extrapolated, 10.0);
    let resized = a.resize(&strat, &view);
}

#[test]
/// Test resizing across several of quantization levels to get a feel for how
/// it should work.
fn test_scenario() {
    let mut rng = seeded_rng(None);

    // start with a full arc
    let a = Arq::new(Loc::from(0x0), 3, 2u32.pow(32 - 3));

    // aim for coverage between 10 and 12.
    let strat = ArqStrat {
        min_coverage: 10.0,
        buffer: 0.2,
    };

    let arcs: Vec<_> =
        simple_parameterized_generator(&mut rng, 10, 0.0001, ArcLenStrategy::Constant(1.0))
            .into_iter()
            .map(|arc| ArqBounds::from_interval(3, arc.interval()).unwrap())
            .collect();

    let view = PeerView::new(ArqSet::new(arcs));
    let extrapolated = view.extrapolated_coverage(&a.to_bounds());
    assert_eq!(extrapolated, 10.0);

    let resized = a.resize(&strat, &view);
    assert_eq!(resized.power(), 3);
    assert_eq!(resized.count(), 2u32.pow(32 - 3));

    todo!("add more peers and watch it upsample and shrink")
}
