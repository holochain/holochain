//! Tests of arc resizing behavior.

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
fn test_resize() {
    let a = Arq::new(
        Loc::from(0x100),
        4, // 0x10
        32,
    );
    assert_eq!(a.to_interval(), ArcInterval::new(0, 0x200 - 1));

    let other = ArqBounds::from_interval(4, ArcInterval::new(0x0, 0x1000)).unwrap();
    dbg!(&other);
    let others = ArqSet::new(vec![other; 12]);
    // aim for coverage between 10 and 11.
    let strat = ArqStrat {
        min_coverage: 10.0,
        buffer: 0.1,
    };
    let view = PeerView::new(others);
    let extrapolated = view.extrapolated_coverage(&a.to_bounds());
    assert_eq!(extrapolated, 12.0);
    let resized = a.resize(&strat, &view);
}
