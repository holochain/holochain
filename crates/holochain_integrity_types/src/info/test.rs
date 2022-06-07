use super::*;

/// Make a new ScopedZomeTypes.
fn make_scope(ranges: Vec<Range<u8>>) -> ScopedZomeTypes {
    ScopedZomeTypes(
        ranges
            .into_iter()
            .map(|r| GlobalZomeTypeId(r.start)..GlobalZomeTypeId(r.end))
            .collect(),
    )
}

#[test]
/// Check that local types can convert to global types.
fn check_to_global_scope() {
    let scope = make_scope(vec![5..9, 3..5]);
    let check = |local: u8, expect: u8| {
        assert_eq!(
            scope.to_global_scope(local).unwrap(),
            GlobalZomeTypeId(expect)
        );
    };
    check(0, 5);
    check(1, 6);
    check(2, 7);
    check(3, 8);
    check(4, 3);
    check(5, 4);
    let check_none = |local: u8| {
        assert_eq!(scope.to_global_scope(local), None);
    };
    check_none(6);
    check_none(7);

    let scope = make_scope(vec![0..1, 1..2, 2..3, 3..4, 4..5, 6..7, 5..6, 7..10]);
    let check = |local: u8, expect: u8| {
        assert_eq!(
            scope.to_global_scope(local).unwrap(),
            GlobalZomeTypeId(expect)
        );
    };
    check(0, 0);
    check(1, 1);
    check(2, 2);
    check(3, 3);
    check(4, 4);

    check(5, 6);
    check(6, 5);

    check(7, 7);
    check(8, 8);
    check(8, 8);
}

#[test]
/// Check that global types can convert to local types.
fn check_to_local_scope() {
    let scope = make_scope(vec![5..9, 3..5]);
    let check = |global: u8, expect: u8| {
        assert_eq!(
            scope.to_local_scope(global).unwrap(),
            LocalZomeTypeId(expect)
        );
    };
    check(5, 0);
    check(6, 1);
    check(7, 2);
    check(8, 3);
    check(3, 4);
    check(4, 5);
    let check_none = |global: u8| {
        assert_eq!(scope.to_local_scope(global), None);
    };
    check_none(0);
    check_none(1);
    check_none(2);
    check_none(9);
    check_none(10);

    let scope = make_scope(vec![0..1, 1..2, 2..3, 3..4, 4..5, 6..7, 5..6, 7..10]);
    let check = |global: u8, expect: u8| {
        assert_eq!(
            scope.to_local_scope(global).unwrap(),
            LocalZomeTypeId(expect)
        );
    };
    check(0, 0);
    check(1, 1);
    check(2, 2);
    check(3, 3);
    check(4, 4);

    check(6, 5);
    check(5, 6);

    check(7, 7);
    check(8, 8);
    check(8, 8);
}
