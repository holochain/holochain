use crate::test::test_peer_view_alpha::assert_between;
use crate::test::test_peer_view_alpha::even_dist_peers;
use crate::*;

#[test]
#[ignore = "test was never completed"]
fn test_peer_coverage() {
    let strat = PeerStratBeta::default();
    let arc = |c, n, h| {
        let mut arc = DhtArc::from_start_and_half_len(0u32, h);
        arc.update_length(&PeerViewBeta::new(Default::default(), arc, c, n).into());
        (arc.coverage() * 10000.0).round() / 10000.0
    };

    let converge = |arc: &mut DhtArc, peers: &Vec<DhtArc>| {
        for _ in 0..40 {
            let view = strat.view(*arc, peers.as_slice()).into();
            arc.update_length(&view);
        }
    };

    let minimum_arc_size = |num_peers: usize| DEFAULT_MIN_PEERS as f64 * (1.0 / num_peers as f64);

    assert_eq!(arc(0.0, 0, MAX_HALF_LENGTH), 1.0);
    for i in 0..(DEFAULT_MIN_PEERS - 1) {
        assert_eq!(arc(i as f64, i, MAX_HALF_LENGTH), 1.0);
    }
    assert_eq!(
        arc(DEFAULT_MIN_PEERS as f64, DEFAULT_MIN_PEERS, MAX_HALF_LENGTH),
        1.0
    );

    // - Start with half coverage and minimum density
    let mut arc = DhtArc::from_start_and_half_len(0u32, MAX_HALF_LENGTH / 2);
    let peers = even_dist_peers(DEFAULT_MIN_PEERS, &[MAX_HALF_LENGTH]);
    converge(&mut arc, &peers);
    // - Converge to full coverage
    assert_eq!(arc.coverage(), 1.0);

    // - Start with full coverage and over density
    let mut arc = DhtArc::from_start_and_half_len(0u32, MAX_HALF_LENGTH);
    let peers = even_dist_peers(DEFAULT_MIN_PEERS * 4, &[MAX_HALF_LENGTH]);
    converge(&mut arc, &peers);
    // - Converge to half coverage
    assert_between(
        arc.coverage(),
        minimum_arc_size(DEFAULT_MIN_PEERS * 4),
        minimum_arc_size(DEFAULT_MIN_PEERS * 4) + DEFAULT_DELTA_SCALE,
    );

    // - Start with full coverage and low density
    let mut arc = DhtArc::from_start_and_half_len(u32::MAX / 2, MAX_HALF_LENGTH);
    let peers = even_dist_peers(DEFAULT_MIN_PEERS * 2, &[20]);
    converge(&mut arc, &peers);
    // - Converge to a full coverage
    assert_eq!((arc.coverage() * 100.0).round() / 100.0, 1.0);

    //- Start with no coverage and under density
    let mut arc = DhtArc::from_start_and_half_len(u32::MAX / 2, 0);
    let peers = even_dist_peers(DEFAULT_MIN_PEERS * 8, &[MAX_HALF_LENGTH / 10]);
    converge(&mut arc, &peers);
    // - Converge to a full coverage
    assert_eq!((arc.coverage() * 100.0).round() / 100.0, 1.0);

    // - Start with no coverage and full network.
    let mut arc = DhtArc::from_start_and_half_len(u32::MAX / 2, 0);
    let peers = even_dist_peers(1000, &[(MAX_HALF_LENGTH as f64 * 0.1) as u32]);
    converge(&mut arc, &peers);
    // - Converge to a full coverage
    assert_between(arc.coverage(), 0.0, 0.1 + DEFAULT_COVERAGE_BUFFER);

    // - Start with no coverage and an almost full network.
    let mut arc = DhtArc::from_start_and_half_len(u32::MAX / 2, 0);
    let peers = even_dist_peers(999, &[(MAX_HALF_LENGTH as f64 * 0.1) as u32]);
    converge(&mut arc, &peers);
    // - Converge to a full coverage
    assert_between(arc.coverage(), 0.09, 0.1 + DEFAULT_COVERAGE_BUFFER);
}
