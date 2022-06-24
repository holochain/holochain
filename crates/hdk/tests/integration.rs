#![cfg(feature = "mock")]

use hdk::prelude::*;
use holo_hash::hash_type::AnyLinkable;
use Compare::*;

fn zome_and_link_type<T>(t: T) -> (u8, u8)
where
    T: Copy,
    ScopedLinkType: TryFrom<T, Error = WasmError>,
{
    let t: ScopedLinkType = t.try_into().unwrap();
    (t.zome_id.0, t.zome_type.0)
}

fn zome_and_entry_type<T>(t: T) -> (u8, u8)
where
    ScopedEntryDefIndex: TryFrom<T, Error = WasmError>,
{
    let t: ScopedEntryDefIndex = t.try_into().unwrap();
    (t.zome_id.0, t.zome_type.0)
}

fn scoped_link(zome_id: u8, link_type: u8) -> ScopedLinkType {
    ScopedZomeType {
        zome_id: zome_id.into(),
        zome_type: link_type.into(),
    }
}
mod integrity_zomes {
    use holochain_deterministic_integrity::prelude::*;

    #[hdk_entry_helper]
    pub struct A;
    #[hdk_entry_helper]
    pub struct B;
    #[hdk_entry_helper]
    pub struct C;

    pub mod integrity_a {

        use super::*;

        #[hdk_entry_defs(skip_hdk_extern = true)]
        #[unit_enum(UnitFoo)]
        pub enum EntryTypes {
            A(A),
            B(B),
            C(C),
        }

        #[hdk_link_types(skip_no_mangle = true)]
        pub enum LinkTypes {
            A,
            B,
            C,
        }
    }

    pub mod integrity_b {
        use super::*;

        #[hdk_entry_defs(skip_hdk_extern = true)]
        #[unit_enum(UnitFoo)]
        pub enum EntryTypes {
            A(A),
            B(B),
            C(C),
        }

        #[hdk_link_types(skip_no_mangle = true)]
        pub enum LinkTypes {
            A,
            B,
            C,
        }
    }
}

#[hdk_dependent_entry_types]
enum EntryZomes {
    A(integrity_zomes::integrity_a::EntryTypes),
    B(integrity_zomes::integrity_b::EntryTypes),
}

#[hdk_dependent_link_types]
enum LinkZomes {
    A(integrity_zomes::integrity_a::LinkTypes),
    B(integrity_zomes::integrity_b::LinkTypes),
}

#[test]
fn combine_integrity_zomes() {
    set_zome_types(&[(22, 3), (12, 3)], &[]);
    create_entry(EntryZomes::A(integrity_zomes::integrity_a::EntryTypes::A(
        integrity_zomes::A {},
    )))
    .unwrap();

    assert_eq!(
        zome_and_entry_type(EntryZomes::A(integrity_zomes::integrity_a::EntryTypes::A(
            integrity_zomes::A {},
        ))),
        (22, 0)
    );
    assert_eq!(
        zome_and_entry_type(EntryZomes::A(integrity_zomes::integrity_a::EntryTypes::B(
            integrity_zomes::B {},
        ))),
        (22, 1)
    );
    assert_eq!(
        zome_and_entry_type(EntryZomes::A(integrity_zomes::integrity_a::EntryTypes::C(
            integrity_zomes::C {},
        ))),
        (22, 2)
    );
    assert_eq!(
        zome_and_entry_type(EntryZomes::B(integrity_zomes::integrity_b::EntryTypes::A(
            integrity_zomes::A {},
        ))),
        (12, 0)
    );
    assert_eq!(
        zome_and_entry_type(EntryZomes::B(integrity_zomes::integrity_b::EntryTypes::B(
            integrity_zomes::B {},
        ))),
        (12, 1)
    );
    assert_eq!(
        zome_and_entry_type(EntryZomes::B(integrity_zomes::integrity_b::EntryTypes::C(
            integrity_zomes::C {},
        ))),
        (12, 2)
    );

    assert!(matches!(
        EntryZomes::deserialize_from_type(12, 0, &Entry::try_from(integrity_zomes::A {}).unwrap()),
        Ok(Some(EntryZomes::B(
            integrity_zomes::integrity_b::EntryTypes::A(integrity_zomes::A {})
        )))
    ));
}

#[test]
fn link_types_create_link() {
    set_zome_types_and_compare(&[], &[(3, 3)], CreateLink(ZomeId(3), LinkType(0)));
    create_link(
        base(),
        base(),
        integrity_zomes::integrity_a::LinkTypes::A,
        (),
    )
    .unwrap();

    assert_eq!(
        zome_and_link_type(integrity_zomes::integrity_a::LinkTypes::A),
        (3, 0)
    );
    assert_eq!(
        zome_and_link_type(integrity_zomes::integrity_a::LinkTypes::B),
        (3, 1)
    );
    assert_eq!(
        zome_and_link_type(integrity_zomes::integrity_a::LinkTypes::C),
        (3, 2)
    );
}

#[test]
fn link_zomes_create_link() {
    use integrity_zomes::*;
    set_zome_types_and_compare(
        &[],
        &[(32, 3), (15, 3)],
        CreateLink(ZomeId(32), LinkType(2)),
    );
    create_link(base(), base(), LinkZomes::A(integrity_a::LinkTypes::C), ()).unwrap();

    set_zome_types_and_compare(
        &[],
        &[(32, 3), (15, 6)],
        CreateLink(ZomeId(15), LinkType(2)),
    );
    create_link(base(), base(), LinkZomes::B(integrity_b::LinkTypes::C), ()).unwrap();

    set_zome_types(&[], &[(3, 3), (2, 6)]);

    assert_eq!(
        zome_and_link_type(LinkZomes::A(integrity_a::LinkTypes::A)),
        (3, 0)
    );
    assert_eq!(
        zome_and_link_type(LinkZomes::A(integrity_a::LinkTypes::B)),
        (3, 1)
    );
    assert_eq!(
        zome_and_link_type(LinkZomes::A(integrity_a::LinkTypes::C)),
        (3, 2)
    );
    assert_eq!(
        zome_and_link_type(LinkZomes::B(integrity_b::LinkTypes::A)),
        (2, 0)
    );
    assert_eq!(
        zome_and_link_type(LinkZomes::B(integrity_b::LinkTypes::B)),
        (2, 1)
    );
    assert_eq!(
        zome_and_link_type(LinkZomes::B(integrity_b::LinkTypes::C)),
        (2, 2)
    );
}

#[test]
fn link_types_get_links() {
    use integrity_zomes::integrity_a::LinkTypes;

    // Include just `A`
    set_zome_types_and_compare(&[], &[(1, 3)], GetLinks(make_filter(&[(1, 0..=0)])));
    get_links(base(), LinkTypes::A, None).unwrap();

    // Include all links from within this zome.
    set_zome_types_and_compare(
        &[],
        &[(1, 3)],
        GetLinks(LinkTypeFilter::single_dep(1.into())),
    );
    get_links(base(), .., None).unwrap();

    // Include types in this vec.
    set_zome_types_and_compare(&[], &[(1, 3)], GetLinks(make_filter(&[(1, 0..=1)])));
    get_links(base(), vec![LinkTypes::A, LinkTypes::B], None).unwrap();

    // Include types in this array.
    set_zome_types_and_compare(
        &[],
        &[(1, 3)],
        GetLinks(LinkTypeFilter::Types(vec![(
            1.into(),
            vec![0.into(), 2.into()],
        )])),
    );
    get_links(base(), [LinkTypes::A, LinkTypes::C], None).unwrap();

    // Include types in this ref to array.
    set_zome_types_and_compare(
        &[],
        &[(1, 3)],
        GetLinks(LinkTypeFilter::Types(vec![(
            1.into(),
            vec![1.into(), 2.into()],
        )])),
    );
    get_links(base(), &[LinkTypes::C, LinkTypes::B], None).unwrap();

    // Include types in this slice.
    let t = [LinkTypes::A, LinkTypes::C];
    set_zome_types_and_compare(
        &[],
        &[(1, 3)],
        GetLinks(LinkTypeFilter::Types(vec![(
            1.into(),
            vec![0.into(), 2.into()],
        )])),
    );
    get_links(base(), &t[..], None).unwrap();
}

#[test]
fn link_zomes_get_links() {
    use integrity_zomes::*;

    // Include just `A(B)`
    set_zome_types_and_compare(
        &[],
        &[(3, 3), (1, 3)],
        GetLinks(LinkTypeFilter::Types(vec![(3.into(), vec![1.into()])])),
    );
    get_links(base(), LinkZomes::A(integrity_a::LinkTypes::B), None).unwrap();

    // Include just `B(B)`
    set_zome_types_and_compare(
        &[],
        &[(3, 3), (1, 3)],
        GetLinks(LinkTypeFilter::Types(vec![(1.into(), vec![1.into()])])),
    );
    get_links(base(), LinkZomes::B(integrity_b::LinkTypes::B), None).unwrap();

    // Include all links from within this zome.
    set_zome_types_and_compare(
        &[],
        &[(3, 3), (1, 3)],
        GetLinks(LinkTypeFilter::Dependencies(vec![3.into(), 1.into()])),
    );
    get_links(base(), .., None).unwrap();

    // Include types in this vec.
    set_zome_types_and_compare(
        &[],
        &[(3, 3), (1, 3)],
        GetLinks(LinkTypeFilter::Types(vec![
            (1.into(), vec![0.into(), 1.into()]),
            (3.into(), vec![0.into(), 1.into()]),
        ])),
    );
    get_links(
        base(),
        vec![
            LinkZomes::A(integrity_a::LinkTypes::A),
            LinkZomes::B(integrity_b::LinkTypes::A),
            LinkZomes::B(integrity_b::LinkTypes::B),
            LinkZomes::A(integrity_a::LinkTypes::B),
        ],
        None,
    )
    .unwrap();

    // Include types in this array.
    set_zome_types_and_compare(
        &[],
        &[(3, 3), (1, 3)],
        GetLinks(LinkTypeFilter::Types(vec![
            (1.into(), vec![0.into()]),
            (3.into(), vec![2.into()]),
        ])),
    );
    get_links(
        base(),
        [
            LinkZomes::B(integrity_b::LinkTypes::A),
            LinkZomes::A(integrity_a::LinkTypes::C),
        ],
        None,
    )
    .unwrap();

    // Include types in this ref to array.
    set_zome_types_and_compare(
        &[],
        &[(3, 3), (1, 3)],
        GetLinks(LinkTypeFilter::Types(vec![
            (1.into(), vec![0.into()]),
            (3.into(), vec![2.into()]),
        ])),
    );
    get_links(
        base(),
        &[
            LinkZomes::B(integrity_b::LinkTypes::A),
            LinkZomes::A(integrity_a::LinkTypes::C),
        ],
        None,
    )
    .unwrap();

    // Include types in this slice.
    let t = [
        LinkZomes::A(integrity_a::LinkTypes::A),
        LinkZomes::A(integrity_a::LinkTypes::C),
    ];
    set_zome_types_and_compare(
        &[],
        &[(3, 3), (1, 3)],
        GetLinks(LinkTypeFilter::Types(vec![(
            3.into(),
            vec![0.into(), 2.into()],
        )])),
    );
    get_links(base(), &t[..], None).unwrap();
}

#[test]
fn link_zomes_from_action() {
    use integrity_zomes::*;
    set_zome_types(&[], &[(19, 3), (4, 3)]);
    assert_eq!(
        LinkZomes::try_from(scoped_link(19, 0)),
        Ok(LinkZomes::A(integrity_a::LinkTypes::A))
    );
    assert_eq!(
        LinkZomes::try_from(scoped_link(19, 1)),
        Ok(LinkZomes::A(integrity_a::LinkTypes::B))
    );
    assert_eq!(
        LinkZomes::try_from(scoped_link(19, 2)),
        Ok(LinkZomes::A(integrity_a::LinkTypes::C))
    );
    assert_eq!(
        LinkZomes::try_from(scoped_link(4, 0)),
        Ok(LinkZomes::B(integrity_b::LinkTypes::A))
    );
    assert_eq!(
        LinkZomes::try_from(scoped_link(4, 1)),
        Ok(LinkZomes::B(integrity_b::LinkTypes::B))
    );
    assert_eq!(
        LinkZomes::try_from(scoped_link(4, 2)),
        Ok(LinkZomes::B(integrity_b::LinkTypes::C))
    );

    assert!(matches!(
        LinkZomes::try_from(ScopedLinkType {
            zome_id: 4.into(),
            zome_type: 50.into()
        }),
        Err(_)
    ));
    assert!(matches!(
        LinkZomes::try_from(ScopedLinkType {
            zome_id: 5.into(),
            zome_type: 2.into()
        }),
        Err(_)
    ));
}

enum Compare {
    GetLinks(LinkTypeFilter),
    CreateLink(ZomeId, LinkType),
    Nothing,
}

fn make_filter(r: &[(u8, std::ops::RangeInclusive<u8>)]) -> LinkTypeFilter {
    LinkTypeFilter::Types(
        r.iter()
            .map(|(z, r)| {
                (
                    ZomeId(*z),
                    r.clone().map(|t| LinkType(t)).collect::<Vec<_>>(),
                )
            })
            .collect(),
    )
}

fn base() -> AnyLinkableHash {
    AnyLinkableHash::from_raw_36_and_type(vec![0; 36], AnyLinkable::External)
}

fn set_zome_types(entries: &[(u8, u8)], links: &[(u8, u8)]) {
    set_zome_types_and_compare(entries, links, Compare::Nothing)
}

fn set_zome_types_and_compare(entries: &[(u8, u8)], links: &[(u8, u8)], compare: Compare) {
    let mut mock_hdk = MockHdkT::new();
    let entries = entries.to_vec();
    let links = links.to_vec();
    mock_hdk.expect_zome_info().returning(move |_| {
        let zome_types = ScopedZomeTypesSet {
            entries: ScopedZomeTypes(
                entries
                    .iter()
                    .map(|(z, types)| (ZomeId(*z), (0..*types).map(|t| EntryDefIndex(t)).collect()))
                    .collect(),
            ),
            links: ScopedZomeTypes(
                links
                    .iter()
                    .map(|(z, types)| (ZomeId(*z), (0..*types).map(|t| LinkType(t)).collect()))
                    .collect(),
            ),
        };

        let info = ZomeInfo {
            name: String::default().into(),
            id: u8::default().into(),
            properties: Default::default(),
            entry_defs: EntryDefs(Default::default()),
            extern_fns: Default::default(),
            zome_types: zome_types.clone(),
        };
        Ok(info)
    });
    mock_hdk
        .expect_create()
        .returning(|_| Ok(ActionHash::from_raw_36(vec![0u8; 36])));
    if !matches!(compare, Compare::CreateLink(_, _)) {
        mock_hdk
            .expect_create_link()
            .returning(|_| Ok(ActionHash::from_raw_36(vec![0u8; 36])));
    }
    if !matches!(compare, Compare::GetLinks(_)) {
        mock_hdk.expect_get_links().returning(|_| {
            Ok(vec![vec![Link {
                target: base(),
                timestamp: Timestamp(0),
                zome_id: 0.into(),
                link_type: 0.into(),
                tag: ().into(),
                create_link_hash: ActionHash::from_raw_36(vec![0u8; 36]),
            }]])
        });
    }
    match compare {
        Compare::GetLinks(l) => {
            mock_hdk
                .expect_get_links()
                .withf(move |input| {
                    input
                        .into_iter()
                        .all(|GetLinksInput { link_type, .. }| *link_type == l)
                })
                .returning(|_| {
                    Ok(vec![vec![Link {
                        target: base(),
                        timestamp: Timestamp(0),
                        zome_id: 0.into(),
                        link_type: 0.into(),
                        tag: ().into(),
                        create_link_hash: ActionHash::from_raw_36(vec![0u8; 36]),
                    }]])
                });
        }
        Compare::CreateLink(z, l) => {
            mock_hdk
                .expect_create_link()
                .withf(
                    move |CreateLinkInput {
                              link_type, zome_id, ..
                          }| *link_type == l && *zome_id == z,
                )
                .returning(|_| Ok(ActionHash::from_raw_36(vec![0u8; 36])));
        }
        Compare::Nothing => (),
    }
    set_hdk(mock_hdk);
}
