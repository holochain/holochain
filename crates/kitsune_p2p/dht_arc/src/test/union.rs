use super::ascii;
use crate::DhtArcSet;

const MAX: u32 = u32::MAX;

macro_rules! assert_union {
    ($a: expr, $b: expr, $e: expr $(,)?) => {
        assert_eq!(DhtArcSet::union(&$a, &$b), $e);
        assert_eq!(DhtArcSet::union(&$b, &$a), $e);
    };
}

#[test]
fn test_union_at_limits() {

    assert_union!(
        DhtArcSet::from(vec![(0, MAX / 2)]),
        DhtArcSet::from(vec![(MAX / 2, MAX)]),
        DhtArcSet::from(vec![(0, MAX),]),
    );
    assert_union!(
        DhtArcSet::from(vec![(0, MAX / 2)]),
        DhtArcSet::from(vec![(MAX / 2, MAX - 1)]),
        DhtArcSet::from(vec![(0, MAX - 1),]),
    );
    assert_union!(
        DhtArcSet::from(vec![(0, MAX / 2)]),
        DhtArcSet::from(vec![(MAX / 2, MAX - 1)]),
        DhtArcSet::from(vec![(0, MAX - 1),]),
    );
    assert_union!(
        DhtArcSet::from(vec![(0, MAX / 2)]),
        DhtArcSet::from(vec![(MAX / 2, MAX - 2)]),
        DhtArcSet::from(vec![(0, MAX - 2),]),
    );
}

#[test]
fn test_union() {
    assert_union!(
        ascii("          "),
        ascii("          "),
        ascii("          "),
    );
    assert_union!(
        ascii("    o     "),
        ascii("     o    "),
        ascii("    oo    "),
    );
    assert_union!(
        ascii("o o o o o "),
        ascii(" o o o o o"),
        ascii("oooooooooo"),
    );
    assert_union!(
        ascii("oooooooooo"),
        ascii("          "),
        ascii("oooooooooo"),
    );
}
