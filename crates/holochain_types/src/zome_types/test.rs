use super::*;
use test_case::test_case;

fn make_set(entries: &[(u8, u8)], links: &[(u8, u8)]) -> GlobalZomeTypes {
    let entries = entries.into_iter().map(|(z, l)| (ZomeId(*z), *l)).collect();
    let links = links.into_iter().map(|(z, l)| (ZomeId(*z), *l)).collect();
    GlobalZomeTypes { entries, links }
}

fn make_scope(entries: &[(u8, u8)], links: &[(u8, u8)]) -> ScopedZomeTypesSet {
    let entries = entries
        .into_iter()
        .map(|(z, l)| (ZomeId(*z), (0..*l).map(|t| t.into()).collect()))
        .collect();
    let links = links
        .into_iter()
        .map(|(z, l)| (ZomeId(*z), (0..*l).map(|t| t.into()).collect()))
        .collect();
    ScopedZomeTypesSet {
        entries: ScopedZomeTypes(entries),
        links: ScopedZomeTypes(links),
    }
}

#[test_case(vec![] => make_set(&[], &[]))]
#[test_case(vec![(0,0)] => make_set(&[(0, 0)], &[(0, 0)]))]
#[test_case(vec![(0,0), (0,0)] => make_set(&[(0, 0), (1, 0)], &[(0, 0), (1, 0)]))]
#[test_case(vec![(1,0)] => make_set(&[(0, 1)], &[(0, 0)]))]
#[test_case(vec![(1,20)] => make_set(&[(0, 1)], &[(0, 20)]))]
#[test_case(vec![(1,20), (0, 0)] => make_set(&[(0, 1), (1, 0)], &[(0, 20), (1, 0)]))]
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

    expect.entries.insert(ZomeId(0), 3);
    expect.entries.insert(ZomeId(1), 0);
    expect.entries.insert(ZomeId(2), 5);
    expect.entries.insert(ZomeId(3), 12);

    expect.links.insert(ZomeId(0), 2);
    expect.links.insert(ZomeId(1), 0);
    expect.links.insert(ZomeId(2), 1);
    expect.links.insert(ZomeId(3), 0);

    assert_eq!(
        GlobalZomeTypes::from_ordered_iterator(zome_types).unwrap(),
        expect
    )
}

#[test_case(make_set(&[], &[]), &[] => make_scope(&[], &[]))]
#[test_case(make_set(&[], &[]), &[0] => make_scope(&[], &[]))]
#[test_case(make_set(&[(0, 20)], &[(0, 5)]), &[0] => make_scope(&[(0, 20)], &[(0, 5)]))]
#[test_case(make_set(&[(0, 20), (1, 10)], &[(0, 5), (1, 10)]), &[0] => make_scope(&[(0, 20)], &[(0, 5)]))]
#[test_case(make_set(&[(0, 20), (1, 10)], &[(0, 5), (1, 10)]), &[1] => make_scope(&[(1, 10)], &[(1, 10)]))]
#[test_case(make_set(&[(0, 20), (1, 10), (2, 15)], &[(0, 5), (1, 10), (2, 3)]), &[1] => make_scope(&[(1, 10)], &[(1, 10)]))]
#[test_case(make_set(&[(0, 20), (1, 10), (2, 15)], &[(0, 5), (1, 10), (2, 3)]), &[1, 2] => make_scope(&[(1, 10), (2, 15)], &[(1, 10), (2, 3)]))]
#[test_case(make_set(&[(0, 20), (1, 10), (2, 15)], &[(0, 5), (1, 10), (2, 3)]), &[2, 1] => make_scope(&[(2, 15), (1, 10)], &[(2, 3), (1, 10)]))]
#[test_case(make_set(&[(0, 20), (1, 10), (2, 15)], &[(0, 5), (1, 10), (2, 3)]), &[0, 2] => make_scope(&[(0, 20), (2, 15)], &[(0, 5), (2, 3)]))]
fn test_in_scope_subset(set: GlobalZomeTypes, zomes: &[u8]) -> ScopedZomeTypesSet {
    let zomes = zomes.iter().map(|z| ZomeId(*z)).collect::<Vec<_>>();
    set.in_scope_subset(&zomes[..])
}
