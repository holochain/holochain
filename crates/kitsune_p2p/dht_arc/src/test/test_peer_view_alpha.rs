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
fn test_check_for_gaps() {
    // Gaps
    assert!(check_for_gaps(vec![DhtArc::from_start_and_half_len(0, 1)]));
    assert!(check_for_gaps(vec![DhtArc::from_start_and_half_len(0, 0)]));
    assert!(check_for_gaps(vec![DhtArc::from_start_and_half_len(
        0,
        MAX_HALF_LENGTH - 2
    )]));
    assert!(check_for_gaps(vec![
        DhtArc::from_start_and_half_len(0u32, MAX_HALF_LENGTH / 2),
        DhtArc::from_start_and_half_len(0u32, MAX_HALF_LENGTH / 2)
    ]));
    assert!(check_for_gaps(vec![
        DhtArc::from_start_and_half_len(0u32, MAX_HALF_LENGTH / 3 + 1),
        DhtArc::from_start_and_half_len(MAX_HALF_LENGTH / 3, MAX_HALF_LENGTH / 3 + 1),
        DhtArc::from_start_and_half_len((MAX_HALF_LENGTH / 3) * 2 + 100, MAX_HALF_LENGTH / 3 + 1)
    ]));
    assert!(check_for_gaps(vec![
        DhtArc::from_start_and_half_len(0u32, MAX_HALF_LENGTH / 3 + 1),
        DhtArc::from_start_and_half_len(MAX_HALF_LENGTH / 3 - 1, MAX_HALF_LENGTH / 3 + 1),
        DhtArc::from_start_and_half_len(
            MAX_HALF_LENGTH + (MAX_HALF_LENGTH / 3),
            MAX_HALF_LENGTH / 2 + MAX_HALF_LENGTH / 8,
        ),
    ]));

    // No Gaps
    assert!(!check_for_gaps(vec![DhtArc::from_start_and_half_len(
        0,
        MAX_HALF_LENGTH
    )]));
    assert!(!check_for_gaps(vec![
        DhtArc::from_start_and_half_len(0u32, MAX_HALF_LENGTH / 2 + 1),
        DhtArc::from_start_and_half_len(MAX_HALF_LENGTH, MAX_HALF_LENGTH / 2 + 1)
    ]));
    assert!(!check_for_gaps(vec![
        DhtArc::from_start_and_half_len(0u32, MAX_HALF_LENGTH / 3 + 1),
        DhtArc::from_start_and_half_len(MAX_HALF_LENGTH / 3 - 1, MAX_HALF_LENGTH / 3 + 1),
        DhtArc::from_start_and_half_len(
            MAX_HALF_LENGTH + (MAX_HALF_LENGTH / 8) - 100,
            MAX_HALF_LENGTH / 2 + MAX_HALF_LENGTH / 8,
        )
    ]));
}

#[test]
fn test_check_redundancy() {
    // Gaps
    assert_eq!(
        check_redundancy(vec![DhtArc::from_start_and_half_len(0u32, 1)]),
        0
    );
    assert_eq!(
        check_redundancy(vec![DhtArc::from_start_and_half_len(0u32, 0)]),
        0
    );
    assert_eq!(
        check_redundancy(vec![DhtArc::from_start_and_half_len(
            0u32,
            MAX_HALF_LENGTH - 2
        )]),
        0
    );
    assert_eq!(
        check_redundancy(vec![
            DhtArc::from_start_and_half_len(0u32, MAX_HALF_LENGTH / 2),
            DhtArc::from_start_and_half_len(0u32, MAX_HALF_LENGTH / 2)
        ]),
        0
    );
    assert_eq!(
        check_redundancy(vec![
            DhtArc::from_start_and_half_len(0u32, MAX_HALF_LENGTH / 3 + 1),
            DhtArc::from_start_and_half_len(MAX_HALF_LENGTH / 3, MAX_HALF_LENGTH / 3 + 1),
            DhtArc::from_start_and_half_len(
                (MAX_HALF_LENGTH / 3) * 2 + 100,
                MAX_HALF_LENGTH / 3 + 1
            )
        ]),
        0
    );
    assert_eq!(
        check_redundancy(vec![
            DhtArc::from_start_and_half_len(0u32, MAX_HALF_LENGTH / 3 + 1),
            DhtArc::from_start_and_half_len(MAX_HALF_LENGTH / 3 - 1, MAX_HALF_LENGTH / 3 + 1),
            DhtArc::from_start_and_half_len(
                MAX_HALF_LENGTH + (MAX_HALF_LENGTH / 3),
                MAX_HALF_LENGTH / 2 + MAX_HALF_LENGTH / 8,
            ),
        ]),
        0
    );

    // No Gaps
    assert_eq!(
        check_redundancy(vec![DhtArc::from_start_and_half_len(0u32, MAX_HALF_LENGTH)]),
        1
    );
    assert_eq!(
        check_redundancy(vec![
            DhtArc::from_start_and_half_len(0u32, MAX_HALF_LENGTH);
            3
        ]),
        3
    );
    assert_eq!(
        check_redundancy(vec![DhtArc::from_start_and_half_len(
            0u32,
            MAX_HALF_LENGTH - 1
        )]),
        1
    );
    assert_eq!(
        check_redundancy(vec![
            DhtArc::from_start_and_half_len(0u32, MAX_HALF_LENGTH / 2 + 1),
            DhtArc::from_start_and_half_len(MAX_HALF_LENGTH, MAX_HALF_LENGTH / 2 + 1)
        ]),
        1
    );
    assert_eq!(
        check_redundancy(vec![
            DhtArc::from_start_and_half_len(0u32, MAX_HALF_LENGTH / 3 + 1),
            DhtArc::from_start_and_half_len(MAX_HALF_LENGTH / 3 - 1, MAX_HALF_LENGTH / 3 + 1),
            DhtArc::from_start_and_half_len(
                MAX_HALF_LENGTH + (MAX_HALF_LENGTH / 8) - 100,
                MAX_HALF_LENGTH / 2 + MAX_HALF_LENGTH / 8,
            )
        ]),
        1
    );
    let arm = Wrapping(MAX_HALF_LENGTH / 3);
    let mut peers = Vec::new();
    for i in 0..12 {
        peers.push(DhtArc::from_start_and_half_len(
            (arm * Wrapping(i)).0,
            arm.0 + 10,
        ));
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
                assert!(!check_for_gaps(peers.clone()), "{}", bucket.to_ascii(64));
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
            DhtArc::from_start_and_half_len(dist as u32, *half_len)
        })
        .collect()
}

pub(crate) fn assert_between(v: f64, lo: f64, hi: f64) {
    assert!(lo <= v && v <= hi, "{} <= {} <= {}", lo, v, hi);
}
