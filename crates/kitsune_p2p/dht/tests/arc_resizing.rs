use kitsune_p2p_dht::arq::*;
use kitsune_p2p_dht::op::*;
use kitsune_p2p_dht_arc::ArcInterval;

#[test]
fn test_resize() {
    let a = Arq::new(
        Loc::from(0x100),
        4, // 0x10
        32,
    );
    {
        let (left, right) = a.boundary_chunks().unwrap();
        assert_eq!(left.left(), 0);
        assert_eq!(right.right(), 0x200 - 1);
    }
    let other = ArqBounds::from_interval(4, ArcInterval::new(0x0, 0x1000)).unwrap();
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
