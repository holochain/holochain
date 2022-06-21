use super::*;

fn make_zome_ids(ids: Vec<(u8, u8)>) -> ScopedZomeTypes {
    let mut total = 0;
    ScopedZomeTypes(
        ids.into_iter()
            .map(|(zome_id, len)| {
                let l = len + total;
                total += len;
                (l.into(), ZomeId(zome_id))
            })
            .collect(),
    )
}

#[test]
fn zome_id_to_local() {
    let map = make_zome_ids(vec![(5, 4), (2, 3)]);

    let check = |zome_id: u8, local: u8| {
        assert!(map.in_scope(local, zome_id));
    };

    check(5, 0);
    check(5, 1);
    check(5, 2);
    check(5, 3);

    check(2, 4);
    check(2, 5);
    check(2, 6);

    let check_not = |zome_id: u8, local: u8| {
        assert!(!map.in_scope(local, zome_id));
    };

    check_not(0, 0);
    check_not(1, 0);
    check_not(3, 0);
    check_not(4, 0);
    check_not(6, 0);

    check_not(5, 4);
    check_not(5, 5);

    check_not(2, 3);
    check_not(2, 7);
    check_not(2, 8);
}

#[test]
fn zome_id_is_dependency() {
    let map = make_zome_ids(vec![(5, 4), (2, 3)]);
    let check = |zome_id: u8| {
        assert!(map.is_dependency(zome_id));
    };

    check(5);
    check(2);

    let check_not = |zome_id: u8| {
        assert!(!map.is_dependency(zome_id));
    };

    check_not(0);
    check_not(1);
    check_not(3);
    check_not(4);
    check_not(6);
    check_not(7);
}

#[test]
fn local_to_zome_id() {
    let map = make_zome_ids(vec![(5, 4), (2, 3)]);

    let check = |local: u8, zome_id: u8| {
        assert_eq!(map.zome_id(local).unwrap(), ZomeId(zome_id));
    };

    check(0, 5);
    check(1, 5);
    check(2, 5);
    check(3, 5);

    check(4, 2);
    check(5, 2);
    check(6, 2);

    let check_none = |local: u8| {
        assert_eq!(map.zome_id(local), None);
    };

    check_none(7);
    check_none(8);
}
