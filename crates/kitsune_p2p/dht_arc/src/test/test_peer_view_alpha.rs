use std::num::Wrapping;

use crate::check_redundancy;
use crate::peer_view::gaps::check_for_gaps;

use crate::*;

// TODO: This is a really good place for prop testing

#[test]
fn test_arc_dist() {
    // start at 5 go all the way around the arc anti-clockwise until
    // you reach 5. You will have traveled 5 less then the entire arc plus one
    // for the reserved zero value
    assert_eq!(wrapped_distance(10, 5), 5);
    assert_eq!(wrapped_distance(5, 10), 5);
    assert_eq!(
        wrapped_distance(Wrapping(u32::MAX) + Wrapping(5), u32::MAX),
        5
    );
    assert_eq!(wrapped_distance(0, u32::MAX), 1);
    assert_eq!(wrapped_distance(0, MAX_HALF_LENGTH), MAX_HALF_LENGTH - 2);
}

#[test]
fn test_dht_arc() {
    assert!(!DhtArc::new(0, 0).contains(0));

    assert!(DhtArc::new(0, 1).contains(0));

    assert!(!DhtArc::new(0, 1).contains(1));
    assert!(!DhtArc::new(1, 0).contains(0));
    assert!(!DhtArc::new(1, 0).contains(1));

    assert!(DhtArc::new(1, 1).contains(1));
    assert!(DhtArc::new(0, 2).contains(0));
    assert!(DhtArc::new(0, 2).contains(1));
    assert!(DhtArc::new(0, 2).contains(u32::MAX));

    assert!(!DhtArc::new(0, 2).contains(2));
    assert!(!DhtArc::new(0, 2).contains(3));
    assert!(!DhtArc::new(0, 2).contains(u32::MAX - 1));
    assert!(!DhtArc::new(0, 2).contains(u32::MAX - 2));

    assert!(DhtArc::new(0, 3).contains(2));
    assert!(DhtArc::new(0, 3).contains(u32::MAX - 1));

    assert!(DhtArc::new(0, MAX_HALF_LENGTH).contains(u32::MAX / 2));
    assert!(DhtArc::new(0, MAX_HALF_LENGTH).contains(u32::MAX));
    assert!(DhtArc::new(0, MAX_HALF_LENGTH).contains(0));
    assert!(DhtArc::new(0, MAX_HALF_LENGTH).contains(MAX_HALF_LENGTH));
}

#[test]
fn test_arc_interval_conversion() {
    assert_eq!(
        DhtArc::new(0, MAX_HALF_LENGTH).interval(),
        ArcInterval::Full,
    );
    assert_eq!(DhtArc::new(0, u32::MAX).interval(), ArcInterval::Full,);
    assert_eq!(
        DhtArc::new(0, u32::MAX / 3).interval(),
        ArcInterval::new(u32::MAX - u32::MAX / 3 + 2, u32::MAX / 3 - 1),
    );
    assert_eq!(
        DhtArc::new(0, u32::MAX / 4).interval(),
        ArcInterval::new(u32::MAX - u32::MAX / 4 + 2, u32::MAX / 4 - 1),
    );
    assert_eq!(DhtArc::new(1000, 5).interval(), ArcInterval::new(996, 1004),);
    assert_eq!(DhtArc::new(1000, 0).interval(), ArcInterval::Empty,);
}

#[test]
fn test_arc_start_end() {
    use std::ops::Bound::*;

    let quarter = (u32::MAX as f64 / 4.0).round() as u32;
    let half = (u32::MAX as f64 / 2.0).round() as u32;

    // Checks that the range is contained and the outside of the range isn't contained
    let check_bounds = |mid, hl, start, end| {
        let out_l = (Wrapping(start) - Wrapping(1u32)).0;
        let out_r = (Wrapping(end) + Wrapping(1u32)).0;
        let opp = (Wrapping(mid) + Wrapping(half)).0;

        assert!(!DhtArc::new(mid, hl).contains(out_l));
        assert!(DhtArc::new(mid, hl).contains(start));
        assert!(DhtArc::new(mid, hl).contains(mid));
        assert!(DhtArc::new(mid, hl).contains(end));
        assert!(!DhtArc::new(mid, hl).contains(out_r));
        assert!(!DhtArc::new(mid, hl + 1).contains(opp));
    };

    // Checks that everything is contained because this is a full range
    let check_bounds_full = |mid, hl, start, end| {
        let out_l = (Wrapping(start) - Wrapping(1u32)).0;
        let out_r = (Wrapping(end) + Wrapping(1u32)).0;
        let opp = (Wrapping(mid) + Wrapping(half)).0;

        assert!(DhtArc::new(mid, hl).contains(out_l));
        assert!(DhtArc::new(mid, hl).contains(start));
        assert!(DhtArc::new(mid, hl).contains(mid));
        assert!(DhtArc::new(mid, hl).contains(end));
        assert!(DhtArc::new(mid, hl).contains(out_r));
        assert!(DhtArc::new(mid, hl + 1).contains(opp));
    };

    assert!(DhtArc::new(0, 0).range().is_empty());
    assert_eq!(DhtArc::new(0, 1).range().into_inc(), 0..=0);
    assert_eq!(DhtArc::new(1, 2).range().into_inc(), 0..=2);
    assert_eq!(
        DhtArc::new(quarter, quarter + 1).range().into_inc(),
        0..=(half as usize)
    );
    check_bounds(quarter, quarter + 1, 0, half);

    assert_eq!(
        DhtArc::new(half, quarter + 1).range().into_inc(),
        (quarter as usize)..=((quarter * 3) as usize)
    );
    check_bounds(half, quarter + 1, quarter, quarter * 3);

    assert_eq!(
        DhtArc::new(half, MAX_HALF_LENGTH).range().into_inc(),
        0..=(u32::MAX as usize)
    );
    check_bounds_full(half, MAX_HALF_LENGTH, 0, u32::MAX);

    // Note the trade of here where we actually redundantly hold
    // position 0.
    assert_eq!(
        DhtArc::new(half, MAX_HALF_LENGTH - 1).range().into_inc(),
        0..=(u32::MAX as usize)
    );
    check_bounds_full(half, MAX_HALF_LENGTH, 1, u32::MAX);

    assert_eq!(
        DhtArc::new(half, MAX_HALF_LENGTH - 2).range().into_inc(),
        2..=((u32::MAX - 1) as usize)
    );
    check_bounds(half, MAX_HALF_LENGTH - 2, 2, u32::MAX - 1);

    assert_eq!(
        DhtArc::new(0, 2).range(),
        ArcRange {
            start: Included(u32::MAX),
            end: Included(1)
        }
    );
    check_bounds(0, 2, u32::MAX, 1);

    assert_eq!(
        DhtArc::new(u32::MAX, 2).range(),
        ArcRange {
            start: Included(u32::MAX - 1),
            end: Included(0)
        }
    );
    check_bounds(u32::MAX, 2, u32::MAX - 1, 0);

    assert_eq!(
        DhtArc::new(0, MAX_HALF_LENGTH).range(),
        ArcRange {
            start: Included(half),
            end: Included(half - 1)
        }
    );
    check_bounds_full(0, MAX_HALF_LENGTH, half, half - 1);

    assert_eq!(
        DhtArc::new(0, MAX_HALF_LENGTH - 1).range(),
        ArcRange {
            start: Included(half),
            end: Included(half - 1)
        }
    );
    check_bounds_full(0, MAX_HALF_LENGTH, half + 1, half - 1);
}

#[test]
fn test_arc_len() {
    let quarter = (u32::MAX as f64 / 4.0).round() as u32;
    let half = (u32::MAX as f64 / 2.0).round() as u32;
    assert_eq!(DhtArc::new(half, MAX_HALF_LENGTH).range().len(), U32_LEN);

    assert_eq!(
        DhtArc::new(half + 1, MAX_HALF_LENGTH).range().len(),
        U32_LEN
    );

    assert_eq!(
        DhtArc::new(half - 1, MAX_HALF_LENGTH).range().len(),
        U32_LEN
    );

    assert_eq!(DhtArc::new(quarter, MAX_HALF_LENGTH).range().len(), U32_LEN);

    assert_eq!(
        DhtArc::new(half, MAX_HALF_LENGTH - 1).range().len(),
        U32_LEN
    );

    assert_eq!(
        DhtArc::new(half, MAX_HALF_LENGTH - 2).range().len(),
        U32_LEN - 3
    );

    assert_eq!(DhtArc::new(0, MAX_HALF_LENGTH).range().len(), U32_LEN);

    assert_eq!(DhtArc::new(0, MAX_HALF_LENGTH - 1).range().len(), U32_LEN);

    assert_eq!(
        DhtArc::new(0, MAX_HALF_LENGTH - 2).range().len(),
        U32_LEN - 3
    );

    assert_eq!(
        DhtArc::new(0, MAX_HALF_LENGTH - 3).range().len(),
        U32_LEN - 5
    );

    assert_eq!(DhtArc::new(0, 0).range().len(), 0);

    assert_eq!(DhtArc::new(0, 1).range().len(), 1);

    assert_eq!(DhtArc::new(0, 2).range().len(), 3);

    assert_eq!(DhtArc::new(0, 3).range().len(), 5);
}

#[test]
#[ignore = "too brittle"]
fn test_peer_density() {
    let strat = PeerStratAlpha::default();
    let arc = |c, n, h| {
        let mut arc = DhtArc::new(0, h);
        arc.update_length(PeerViewAlpha::new(Default::default(), arc, c, n));
        (arc.coverage() * 10000.0).round() / 10000.0
    };

    let converge = |arc: &mut DhtArc, peers: &Vec<DhtArc>| {
        for _ in 0..40 {
            let view = strat.view(*arc, peers.as_slice());
            arc.update_length(view);
        }
    };

    assert_eq!(arc(0.0, 0, MAX_HALF_LENGTH), 1.0);
    for i in 0..(DEFAULT_MIN_PEERS - 1) {
        assert_eq!(arc(1.0, i, MAX_HALF_LENGTH), 1.0);
    }
    assert_eq!(arc(1.0, DEFAULT_MIN_PEERS, MAX_HALF_LENGTH), 1.0);

    // - Start with half coverage and minimum density
    let mut arc = DhtArc::new(0, MAX_HALF_LENGTH / 2);
    let peers = even_dist_peers(DEFAULT_MIN_PEERS, &[MAX_HALF_LENGTH]);
    converge(&mut arc, &peers);
    // - Converge to full coverage
    assert_eq!(arc.coverage(), 1.0);

    // - Start with full coverage and over density
    let mut arc = DhtArc::new(0, MAX_HALF_LENGTH);
    let peers = even_dist_peers(DEFAULT_MIN_PEERS * 2, &[MAX_HALF_LENGTH]);
    converge(&mut arc, &peers);
    // - Converge to half coverage
    assert_between((arc.coverage() * 10.0).round() / 10.0, 0.5, 0.6);

    // - Start with full coverage and low density
    let mut arc = DhtArc::new(u32::MAX / 2, MAX_HALF_LENGTH);
    let peers = even_dist_peers(DEFAULT_MIN_PEERS * 2, &[20]);
    converge(&mut arc, &peers);
    // - Converge to a full coverage
    assert_eq!((arc.coverage() * 100.0).round() / 100.0, 1.0);

    // - Start with no coverage and under density
    let mut arc = DhtArc::new(u32::MAX / 2, 0);
    let peers = even_dist_peers(DEFAULT_MIN_PEERS * 8, &[MAX_HALF_LENGTH / 10]);
    converge(&mut arc, &peers);
    // - Converge to a full coverage
    assert_eq!((arc.coverage() * 100.0).round() / 100.0, 1.0);
}

#[test]
#[ignore = "too brittle"]
fn test_converge() {
    let strat = PeerStratAlpha::default();
    let min_online_peers = DEFAULT_MIN_PEERS;
    let bucket = DhtArc::new(0, MAX_HALF_LENGTH);
    assert_eq!(
        (PeerViewAlpha::new(strat, bucket, 1.0, 1).next_coverage(1.0)),
        1.0
    );
    assert_eq!(
        (PeerViewAlpha::new(strat, bucket, 1.0, min_online_peers).next_coverage(1.0)),
        1.0
    );
    assert_between(
        PeerViewAlpha::new(strat, bucket, 1.0, min_online_peers * 2).next_coverage(1.0),
        0.9,
        0.91,
    );
    assert_eq!(
        (PeerViewAlpha::new(strat, bucket, 1.0, min_online_peers).next_coverage(0.5)),
        0.6
    );
    let mut coverage = 0.5;
    for _ in 0..20 {
        coverage = PeerViewAlpha::new(strat, bucket, 1.0, 1).next_coverage(coverage);
    }
    assert_eq!(coverage, 1.0);

    let mut coverage = 1.0;
    for _ in 0..20 {
        coverage =
            PeerViewAlpha::new(strat, bucket, 1.0, min_online_peers * 2).next_coverage(coverage);
    }
    assert_between(coverage, 0.5, 0.55);
}

#[test]
#[ignore = "too brittle"]
fn test_multiple() {
    let strat: PeerStrat = PeerStratAlpha::default().into();
    let converge = |peers: &mut Vec<DhtArc>| {
        let mut mature = false;
        for _ in 0..40 {
            for i in 0..peers.len() {
                let p = peers.clone();
                let arc = peers.get_mut(i).unwrap();
                let view = strat.view(*arc, p.as_slice());
                arc.update_length(view);
            }
            let r = check_redundancy(peers.clone());
            if mature {
                assert!(r >= DEFAULT_MIN_REDUNDANCY);
            } else {
                if r >= DEFAULT_MIN_REDUNDANCY {
                    mature = true;
                }
            }
        }
        assert!(mature)
    };

    let mut peers = even_dist_peers(DEFAULT_MIN_PEERS, &[20]);
    converge(&mut peers);
    for arc in peers {
        assert_eq!((arc.coverage() * 100.0).round() / 100.0, 1.0);
    }

    let mut peers = even_dist_peers(DEFAULT_MIN_PEERS, &[MAX_HALF_LENGTH]);
    converge(&mut peers);
    for arc in peers {
        assert_eq!((arc.coverage() * 100.0).round() / 100.0, 1.0);
    }

    let mut peers = even_dist_peers(DEFAULT_MIN_PEERS * 4, &[20]);
    converge(&mut peers);
    for arc in peers {
        let cov = (arc.coverage() * 100.0).round() / 100.0;
        assert!(cov >= 0.25);
        assert!(cov <= 0.3);
    }

    let mut peers = even_dist_peers(DEFAULT_MIN_PEERS * 4, &[MAX_HALF_LENGTH]);
    converge(&mut peers);
    for arc in peers {
        let cov = (arc.coverage() * 100.0).round() / 100.0;
        assert!(cov >= 0.25);
        assert!(cov <= 0.3);
    }
}

#[test]
fn test_check_for_gaps() {
    // Gaps
    assert!(check_for_gaps(vec![DhtArc::new(0, 1)]));
    assert!(check_for_gaps(vec![DhtArc::new(0, 0)]));
    assert!(check_for_gaps(vec![DhtArc::new(0, MAX_HALF_LENGTH - 2)]));
    assert!(check_for_gaps(vec![
        DhtArc::new(0, MAX_HALF_LENGTH / 2),
        DhtArc::new(0, MAX_HALF_LENGTH / 2)
    ]));
    assert!(check_for_gaps(vec![
        DhtArc::new(0, MAX_HALF_LENGTH / 3 + 1),
        DhtArc::new(MAX_HALF_LENGTH / 3, MAX_HALF_LENGTH / 3 + 1),
        DhtArc::new((MAX_HALF_LENGTH / 3) * 2 + 100, MAX_HALF_LENGTH / 3 + 1)
    ]));
    assert!(check_for_gaps(vec![
        DhtArc::new(0, MAX_HALF_LENGTH / 3 + 1),
        DhtArc::new(MAX_HALF_LENGTH / 3 - 1, MAX_HALF_LENGTH / 3 + 1),
        DhtArc::new(
            MAX_HALF_LENGTH + (MAX_HALF_LENGTH / 3),
            MAX_HALF_LENGTH / 2 + MAX_HALF_LENGTH / 8,
        ),
    ]));

    // No Gaps
    assert!(!check_for_gaps(vec![DhtArc::new(0, MAX_HALF_LENGTH)]));
    assert!(!check_for_gaps(vec![
        DhtArc::new(0, MAX_HALF_LENGTH / 2 + 1),
        DhtArc::new(MAX_HALF_LENGTH, MAX_HALF_LENGTH / 2 + 1)
    ]));
    assert!(!check_for_gaps(vec![
        DhtArc::new(0, MAX_HALF_LENGTH / 3 + 1),
        DhtArc::new(MAX_HALF_LENGTH / 3 - 1, MAX_HALF_LENGTH / 3 + 1),
        DhtArc::new(
            MAX_HALF_LENGTH + (MAX_HALF_LENGTH / 8) - 100,
            MAX_HALF_LENGTH / 2 + MAX_HALF_LENGTH / 8,
        )
    ]));
}

#[test]
fn test_check_redundancy() {
    // Gaps
    assert_eq!(check_redundancy(vec![DhtArc::new(0, 1)]), 0);
    assert_eq!(check_redundancy(vec![DhtArc::new(0, 0)]), 0);
    assert_eq!(
        check_redundancy(vec![DhtArc::new(0, MAX_HALF_LENGTH - 2)]),
        0
    );
    assert_eq!(
        check_redundancy(vec![
            DhtArc::new(0, MAX_HALF_LENGTH / 2),
            DhtArc::new(0, MAX_HALF_LENGTH / 2)
        ]),
        0
    );
    assert_eq!(
        check_redundancy(vec![
            DhtArc::new(0, MAX_HALF_LENGTH / 3 + 1),
            DhtArc::new(MAX_HALF_LENGTH / 3, MAX_HALF_LENGTH / 3 + 1),
            DhtArc::new((MAX_HALF_LENGTH / 3) * 2 + 100, MAX_HALF_LENGTH / 3 + 1)
        ]),
        0
    );
    assert_eq!(
        check_redundancy(vec![
            DhtArc::new(0, MAX_HALF_LENGTH / 3 + 1),
            DhtArc::new(MAX_HALF_LENGTH / 3 - 1, MAX_HALF_LENGTH / 3 + 1),
            DhtArc::new(
                MAX_HALF_LENGTH + (MAX_HALF_LENGTH / 3),
                MAX_HALF_LENGTH / 2 + MAX_HALF_LENGTH / 8,
            ),
        ]),
        0
    );

    // No Gaps
    assert_eq!(check_redundancy(vec![DhtArc::new(0, MAX_HALF_LENGTH)]), 1);
    assert_eq!(
        check_redundancy(vec![DhtArc::new(0, MAX_HALF_LENGTH); 3]),
        3
    );
    assert_eq!(
        check_redundancy(vec![DhtArc::new(0, MAX_HALF_LENGTH - 1)]),
        1
    );
    assert_eq!(
        check_redundancy(vec![
            DhtArc::new(0, MAX_HALF_LENGTH / 2 + 1),
            DhtArc::new(MAX_HALF_LENGTH, MAX_HALF_LENGTH / 2 + 1)
        ]),
        1
    );
    assert_eq!(
        check_redundancy(vec![
            DhtArc::new(0, MAX_HALF_LENGTH / 3 + 1),
            DhtArc::new(MAX_HALF_LENGTH / 3 - 1, MAX_HALF_LENGTH / 3 + 1),
            DhtArc::new(
                MAX_HALF_LENGTH + (MAX_HALF_LENGTH / 8) - 100,
                MAX_HALF_LENGTH / 2 + MAX_HALF_LENGTH / 8,
            )
        ]),
        1
    );
    let arm = Wrapping(MAX_HALF_LENGTH / 3);
    let mut peers = Vec::new();
    for i in 0..12 {
        peers.push(DhtArc::new(arm * Wrapping(i), arm.0 + 10));
    }
    assert_eq!(check_redundancy(peers), 4);
}

#[test]
#[ignore = "too brittle"]
fn test_peer_gaps() {
    let converge = |peers: &mut Vec<DhtArc>| {
        let strat: PeerStrat = PeerStratAlpha {
            check_gaps: true,
            ..Default::default()
        }
        .into();
        let mut gaps = true;
        for _ in 0..40 {
            for i in 0..peers.len() {
                let p = peers.clone();
                let arc = peers.get_mut(i).unwrap();
                let view = strat.view(*arc, p.as_slice());
                arc.update_length(view);
            }
            if gaps {
                gaps = check_for_gaps(peers.clone());
            } else {
                let bucket = DhtArcBucket::new(peers[0].clone(), peers.clone());
                assert!(!check_for_gaps(peers.clone()), "{}", bucket);
            }
        }
    };
    let mut peers = even_dist_peers(DEFAULT_MIN_PEERS * 10, &[MAX_HALF_LENGTH / 4]);
    converge(&mut peers);
    for arc in peers {
        assert_between((arc.coverage() * 100.0).round() / 100.0, 0.1, 0.15);
    }

    let mut peers = even_dist_peers(DEFAULT_MIN_PEERS, &[20]);
    converge(&mut peers);
    for arc in peers {
        assert_eq!((arc.coverage() * 10.0).round() / 10.0, 1.0);
    }
}

pub(crate) fn even_dist_peers(num: usize, half_lens: &[u32]) -> Vec<DhtArc> {
    let mut hl = half_lens.iter();
    let iter = std::iter::repeat_with(|| hl.next().unwrap_or(&half_lens[0]));
    (0..num)
        .zip(iter)
        .map(|(i, half_len)| {
            let dist = i as f64 / num as f64 * u32::MAX as f64;
            DhtArc::new(dist as u32, *half_len)
        })
        .collect()
}

pub(crate) fn assert_between(v: f64, lo: f64, hi: f64) {
    assert!(lo <= v && v <= hi, "{} <= {} <= {}", lo, v, hi);
}
