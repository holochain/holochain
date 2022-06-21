use super::*;
use holochain_zome_types::LocalZomeTypeId;
use test_case::test_case;

fn make_set(entries: &[(u8, u8)], links: &[(u8, u8)]) -> GlobalZomeTypes {
    let entries = entries
        .into_iter()
        .map(|(z, l)| (ZomeId(*z), NonZeroU8::new(*l).unwrap()))
        .collect();
    let links = links
        .into_iter()
        .map(|(z, l)| (ZomeId(*z), NonZeroU8::new(*l).unwrap()))
        .collect();
    GlobalZomeTypes { entries, links }
}

fn make_scope(entries: &[(u8, u8)], links: &[(u8, u8)]) -> ScopedZomeTypesSet {
    let entries = entries
        .into_iter()
        .map(|(z, l)| (LocalZomeTypeId(*z), ZomeId(*l)))
        .collect();
    let links = links
        .into_iter()
        .map(|(z, l)| (LocalZomeTypeId(*z), ZomeId(*l)))
        .collect();
    ScopedZomeTypesSet {
        entries: ScopedZomeTypes(entries),
        links: ScopedZomeTypes(links),
    }
}

#[test_case(vec![] => make_set(&[], &[]))]
#[test_case(vec![(0,0)] => make_set(&[], &[]))]
#[test_case(vec![(0,0), (0,0)] => make_set(&[], &[]))]
#[test_case(vec![(1,0)] => make_set(&[(0, 1)], &[]))]
#[test_case(vec![(1,20)] => make_set(&[(0, 1)], &[(0, 20)]))]
#[test_case(vec![(1,20), (0, 0)] => make_set(&[(0, 1)], &[(0, 20)]))]
fn test_from_ordered_iterator(iter: Vec<(u8, u8)>) -> GlobalZomeTypes {
    GlobalZomeTypes::from_ordered_iterator(
        iter.into_iter()
            .map(|(e, l)| (EntryDefIndex(e), LinkType(l))),
    )
    .unwrap()
}

#[test]
fn test_from_ordered_iterator_err() {
    assert!(matches!(
        GlobalZomeTypes::from_ordered_iterator(
            (0..300)
                .into_iter()
                .map(|_| (EntryDefIndex(1), LinkType(1))),
        )
        .unwrap_err(),
        ZomeTypesError::ZomeIndexOverflow
    ));
}

#[test]
fn construction_is_deterministic() {
    let zome_types = vec![
        (EntryDefIndex(3), LinkType(2)),
        (EntryDefIndex(0), LinkType(0)),
        (EntryDefIndex(5), LinkType(1)),
        (EntryDefIndex(12), LinkType(0)),
    ];

    assert_eq!(
        GlobalZomeTypes::from_ordered_iterator(zome_types.clone()).unwrap(),
        GlobalZomeTypes::from_ordered_iterator(zome_types.clone()).unwrap(),
    );

    let mut expect = GlobalZomeTypes::default();

    expect.entries.insert(ZomeId(0), NonZeroU8::new(3).unwrap());
    expect.entries.insert(ZomeId(2), NonZeroU8::new(5).unwrap());
    expect
        .entries
        .insert(ZomeId(3), NonZeroU8::new(12).unwrap());

    expect.links.insert(ZomeId(0), NonZeroU8::new(2).unwrap());
    expect.links.insert(ZomeId(2), NonZeroU8::new(1).unwrap());

    assert_eq!(
        GlobalZomeTypes::from_ordered_iterator(zome_types).unwrap(),
        expect
    )
}

#[test_case(make_set(&[], &[]), &[] => make_scope(&[], &[]))]
#[test_case(make_set(&[], &[]), &[0] => make_scope(&[], &[]))]
#[test_case(make_set(&[(0, 20)], &[(0, 5)]), &[0] => make_scope(&[(19, 0)], &[(4, 0)]))]
// #[test_case(make_set(&[0..20, 20..30], &[0..5, 5..15]), &[0] => make_set(&[0..20], &[0..5]))]
// #[test_case(make_set(&[0..20, 20..30], &[0..5, 5..15]), &[1] => make_set(&[20..30], &[5..15]))]
// #[test_case(make_set(&[0..20, 20..30, 30..40], &[0..5, 5..15, 0..0]), &[1] => make_set(&[20..30], &[5..15]))]
// #[test_case(make_set(&[0..20, 20..30, 30..40], &[0..5, 5..15, 15..15]), &[1, 2] => make_set(&[20..30, 30..40], &[5..15, 15..15]))]
// #[test_case(make_set(&[0..20, 20..30, 30..40], &[0..5, 5..15, 15..15]), &[0, 2] => make_set(&[0..20, 30..40], &[0..5, 15..15]))]
fn test_re_scope(set: GlobalZomeTypes, zomes: &[u8]) -> ScopedZomeTypesSet {
    let zomes = zomes.iter().map(|z| ZomeId(*z)).collect::<Vec<_>>();
    set.re_scope(&zomes[..]).unwrap()
}

// #[test_case(make_set(&[], &[]), &[0] => matches ZomeTypesError::MissingZomeType(z) if z.0 == 0)]
// #[test_case(make_set(&[0..1, 1..2, 2..3], &[0..1]), &[1] => matches ZomeTypesError::MissingZomeType(z) if z.0 == 1)]
// #[test_case(make_set(&[0..1], &[0..1, 1..2, 2..3]), &[1] => matches ZomeTypesError::MissingZomeType(z) if z.0 == 1)]
// #[test_case(make_set(&[0..1, 1..2, 2..3], &[0..1, 1..2, 2..3]), &[3] => matches ZomeTypesError::MissingZomeType(z) if z.0 == 3)]
// fn test_re_scope_err(set: GlobalZomeTypes, zomes: &[u8]) -> ZomeTypesError {
//     let zomes = zomes.iter().map(|z| ZomeId(*z)).collect::<Vec<_>>();
//     set.re_scope(&zomes[..]).unwrap_err()
// }

// #[test_case(&[0..1], 0 => 0)]
// #[test_case(&[0..1, 1..5], 0 => 0)]
// #[test_case(&[0..2, 2..5], 1 => 0)]
// #[test_case(&[0..2, 2..5], 2 => 1)]
// fn test_find_zome_id(iter: &[Range<u8>], index: u8) -> u8 {
//     let iter: Vec<_> = iter
//         .into_iter()
//         .map(|r| GlobalZomeTypeId(r.start)..GlobalZomeTypeId(r.end))
//         .collect();
//     find_zome_id(iter.iter(), &GlobalZomeTypeId(index))
//         .unwrap()
//         .0
// }

// #[test_case(&[], 0)]
// #[test_case(&[0..1], 1)]
// #[test_case(&[0..2, 2..5], 5)]
// fn test_find_zome_id_none(iter: &[Range<u8>], index: u8) {
//     let iter: Vec<_> = iter
//         .into_iter()
//         .map(|r| GlobalZomeTypeId(r.start)..GlobalZomeTypeId(r.end))
//         .collect();
//     assert_eq!(find_zome_id(iter.iter(), &GlobalZomeTypeId(index)), None);
// }
