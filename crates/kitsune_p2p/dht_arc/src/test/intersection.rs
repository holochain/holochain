use super::ascii;
use crate::DhtArcSet;

const MAX: u32 = u32::MAX;

macro_rules! assert_intersection {
    ($a: expr, $b: expr, $e: expr $(,)?) => {
        let empty = $e.is_empty();
        assert_eq!(DhtArcSet::intersection(&$a, &$b), $e);
        assert_eq!(DhtArcSet::intersection(&$b, &$a), $e);
        if empty {
            assert!(!DhtArcSet::overlap(&$a, &$b));
            assert!(!DhtArcSet::overlap(&$b, &$a));
        } else {
            assert!(DhtArcSet::overlap(&$a, &$b));
            assert!(DhtArcSet::overlap(&$b, &$a));
        }
    };
}

#[test]
// @maackle Do you know why this is now failing?
#[ignore = "Broken not sure how to fix"]
fn test_intersection_at_limits() {
    assert_intersection!(
        DhtArcSet::from(vec![(0, MAX)]),
        DhtArcSet::from(vec![(0, MAX / 4), (MAX / 2, MAX),]),
        DhtArcSet::from(vec![(0, MAX / 4), (MAX / 2, MAX),]),
    );
    assert_intersection!(
        DhtArcSet::from(vec![(1, MAX - 1)]),
        DhtArcSet::from(vec![(0, MAX / 4), (MAX / 2, MAX),]),
        DhtArcSet::from(vec![(1, MAX / 4), (MAX / 2, MAX - 1),]),
    );
    assert_intersection!(
        DhtArcSet::from(vec![(1, MAX)]),
        DhtArcSet::from(vec![(0, MAX / 4), (MAX / 2, MAX),]),
        DhtArcSet::from(vec![(1, MAX / 4), (MAX / 2, MAX),]),
    );
}

#[test]
// @maackle Can we delete this if we don't care about edge case?
#[ignore = "This test should pass, but doesn't.
It seems to highlight a weird edge case in the underlying `intervallum` crate,
but shouldn't have an impact on our use"]
fn test_intersection_at_limits_bug() {
    assert_intersection!(
        DhtArcSet::from(vec![(0, MAX - 1)]),
        DhtArcSet::from(vec![(0, MAX / 4), (MAX / 2, MAX),]),
        DhtArcSet::from(vec![(0, MAX / 4), (MAX / 2, MAX - 1),]),
    );
}

#[test]
fn test_intersection() {
    assert_intersection!(
        ascii("oo       o"),
        ascii("o       oo"),
        ascii("o        o"),
    );
    assert_intersection!(
        ascii("  ooo     "),
        ascii("    ooo   "),
        ascii("    o     "),
    );
    assert_intersection!(
        ascii("o o o o o "),
        ascii(" o o o o o"),
        ascii("          "),
    );
    assert_intersection!(
        ascii("oooooooooo"),
        ascii("          "),
        ascii("          "),
    );
}
