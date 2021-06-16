use super::ascii;
use crate::DhtArcSet;

macro_rules! assert_union {
    ($a: expr, $b: expr, $e: expr $(,)?) => {
        assert_eq!(DhtArcSet::union(&$a, &$b), $e);
        assert_eq!(DhtArcSet::union(&$b, &$a), $e);
    };
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
