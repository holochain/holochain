#![cfg(feature = "mock")]
use std::ops::Range;

use hdk::prelude::*;
use holo_hash::hash_type::AnyLinkable;
use Compare::*;

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

        #[hdk_link_types]
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

        #[hdk_link_types]
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
    set_zome_types(vec![0..3, 3..6], vec![]);
    create_entry(EntryZomes::A(integrity_zomes::integrity_a::EntryTypes::A(
        integrity_zomes::A {},
    )))
    .unwrap();

    assert_eq!(
        EntryDefIndex::try_from(EntryZomes::A(integrity_zomes::integrity_a::EntryTypes::A(
            integrity_zomes::A {},
        ))),
        Ok(EntryDefIndex(0))
    );
    assert_eq!(
        EntryDefIndex::try_from(&EntryZomes::A(integrity_zomes::integrity_a::EntryTypes::A(
            integrity_zomes::A {},
        ))),
        Ok(EntryDefIndex(0))
    );
    assert_eq!(
        EntryDefIndex::try_from(EntryZomes::A(integrity_zomes::integrity_a::EntryTypes::B(
            integrity_zomes::B {},
        ))),
        Ok(EntryDefIndex(1))
    );
    assert_eq!(
        EntryDefIndex::try_from(EntryZomes::A(integrity_zomes::integrity_a::EntryTypes::C(
            integrity_zomes::C {},
        ))),
        Ok(EntryDefIndex(2))
    );

    assert_eq!(
        EntryDefIndex::try_from(EntryZomes::B(integrity_zomes::integrity_b::EntryTypes::A(
            integrity_zomes::A {},
        ))),
        Ok(EntryDefIndex(3))
    );
    assert_eq!(
        EntryDefIndex::try_from(EntryZomes::B(integrity_zomes::integrity_b::EntryTypes::B(
            integrity_zomes::B {},
        ))),
        Ok(EntryDefIndex(4))
    );
    assert_eq!(
        EntryDefIndex::try_from(EntryZomes::B(integrity_zomes::integrity_b::EntryTypes::C(
            integrity_zomes::C {},
        ))),
        Ok(EntryDefIndex(5))
    );

    assert!(matches!(
        EntryZomes::try_from_global_type(0u8, &Entry::try_from(integrity_zomes::A {}).unwrap()),
        Ok(Some(EntryZomes::A(
            integrity_zomes::integrity_a::EntryTypes::A(integrity_zomes::A {})
        )))
    ));
}

#[test]
fn link_types_create_link() {
    set_zome_types_and_compare(vec![], vec![12..15], CreateLink(LinkType(12)));
    create_link(
        base(),
        base(),
        integrity_zomes::integrity_a::LinkTypes::A,
        (),
    )
    .unwrap();

    assert_eq!(
        LinkType::try_from(integrity_zomes::integrity_a::LinkTypes::A),
        Ok(LinkType(12))
    );

    assert_eq!(
        LinkType::try_from(integrity_zomes::integrity_a::LinkTypes::B),
        Ok(LinkType(13))
    );

    assert_eq!(
        LinkType::try_from(integrity_zomes::integrity_a::LinkTypes::C),
        Ok(LinkType(14))
    );
}

#[test]
fn link_zomes_create_link() {
    use integrity_zomes::*;
    set_zome_types_and_compare(vec![], vec![12..15, 0..3], CreateLink(LinkType(14)));
    create_link(base(), base(), LinkZomes::A(integrity_a::LinkTypes::C), ()).unwrap();

    set_zome_types_and_compare(vec![], vec![12..15, 0..3], CreateLink(LinkType(2)));
    create_link(base(), base(), LinkZomes::B(integrity_b::LinkTypes::C), ()).unwrap();

    set_zome_types(vec![], vec![12..15, 0..3]);

    assert_eq!(
        LinkType::try_from(LinkZomes::A(integrity_a::LinkTypes::A)),
        Ok(LinkType(12))
    );

    assert_eq!(
        LinkType::try_from(LinkZomes::A(integrity_a::LinkTypes::B)),
        Ok(LinkType(13))
    );

    assert_eq!(
        LinkType::try_from(LinkZomes::A(integrity_a::LinkTypes::C)),
        Ok(LinkType(14))
    );

    assert_eq!(
        LinkType::try_from(LinkZomes::B(integrity_b::LinkTypes::A)),
        Ok(LinkType(0))
    );

    assert_eq!(
        LinkType::try_from(LinkZomes::B(integrity_b::LinkTypes::B)),
        Ok(LinkType(1))
    );

    assert_eq!(
        LinkType::try_from(LinkZomes::B(integrity_b::LinkTypes::C)),
        Ok(LinkType(2))
    );
}

#[test]
fn link_types_get_links() {
    use integrity_zomes::integrity_a::LinkTypes;

    // Include just `A`
    set_zome_types_and_compare(vec![], vec![12..15], GetLinks(make_range(vec![12..=12])));
    get_links(base(), LinkTypes::A, None).unwrap();

    // Include all links from within this zome.
    set_zome_types_and_compare(vec![], vec![12..15], GetLinks(make_range(vec![12..=14])));
    get_links(base(), LinkTypes::range(..), None).unwrap();

    // Include link types from `A` up
    set_zome_types_and_compare(vec![], vec![12..15], GetLinks(make_range(vec![12..=14])));
    get_links(base(), LinkTypes::range(LinkTypes::A..), None).unwrap();

    // Include link types from `A` to `C` inclusive.
    set_zome_types_and_compare(vec![], vec![12..15], GetLinks(make_range(vec![12..=14])));
    get_links(base(), LinkTypes::range(LinkTypes::A..=LinkTypes::C), None).unwrap();

    // Include no link types (this isn't very useful).
    set_zome_types_and_compare(
        vec![],
        vec![12..15],
        GetLinks(LinkTypeRanges(vec![LinkTypeRange::Empty])),
    );
    get_links(base(), LinkTypes::range(LinkTypes::A..LinkTypes::A), None).unwrap();

    // Include types in this vec.
    set_zome_types_and_compare(
        vec![],
        vec![12..15],
        GetLinks(make_range(vec![12..=12, 13..=13])),
    );
    get_links(base(), vec![LinkTypes::A, LinkTypes::B], None).unwrap();

    // Include types in this array.
    set_zome_types_and_compare(
        vec![],
        vec![12..15],
        GetLinks(make_range(vec![12..=12, 14..=14])),
    );
    get_links(base(), [LinkTypes::A, LinkTypes::C], None).unwrap();

    // Include types in this ref to array.
    set_zome_types_and_compare(
        vec![],
        vec![12..15],
        GetLinks(make_range(vec![14..=14, 13..=13])),
    );
    get_links(base(), &[LinkTypes::C, LinkTypes::B], None).unwrap();

    // Include types in this slice.
    let t = [LinkTypes::A, LinkTypes::C];
    set_zome_types_and_compare(
        vec![],
        vec![12..15],
        GetLinks(make_range(vec![12..=12, 14..=14])),
    );
    get_links(base(), &t[..], None).unwrap();

    // Include all link types defined in any zome.
    set_zome_types_and_compare(
        vec![],
        vec![12..15],
        GetLinks(LinkTypeRanges(vec![LinkTypeRange::Full])),
    );
    get_links(base(), .., None).unwrap();
}

#[test]
fn link_zomes_get_links() {
    use integrity_zomes::*;

    // Include just `A(B)`
    set_zome_types_and_compare(
        vec![],
        vec![12..15, 5..8],
        GetLinks(make_range(vec![13..=13])),
    );
    get_links(base(), LinkZomes::A(integrity_a::LinkTypes::B), None).unwrap();

    // Include just `B(B)`
    set_zome_types_and_compare(
        vec![],
        vec![12..15, 5..8],
        GetLinks(make_range(vec![6..=6])),
    );
    get_links(base(), LinkZomes::B(integrity_b::LinkTypes::B), None).unwrap();

    // Include all links from within this zome.
    set_zome_types_and_compare(
        vec![],
        vec![12..15, 5..8],
        GetLinks(make_range(vec![12..=14, 5..=7])),
    );
    get_links(base(), LinkZomes::range(..), None).unwrap();

    // Include link types from `A(C)` up
    set_zome_types_and_compare(
        vec![],
        vec![12..15, 5..8],
        GetLinks(make_range(vec![14..=14, 5..=7])),
    );
    get_links(
        base(),
        LinkZomes::range(LinkZomes::A(integrity_a::LinkTypes::C)..),
        None,
    )
    .unwrap();

    // Include link types from `A(B)` to `B(A)` inclusive.
    set_zome_types_and_compare(
        vec![],
        vec![12..15, 5..8],
        GetLinks(make_range(vec![13..=14, 5..=5])),
    );
    get_links(
        base(),
        LinkZomes::range(
            LinkZomes::A(integrity_a::LinkTypes::B)..=LinkZomes::B(integrity_b::LinkTypes::A),
        ),
        None,
    )
    .unwrap();

    // Include no link types (this isn't very useful).
    set_zome_types_and_compare(
        vec![],
        vec![12..15, 5..8],
        GetLinks(LinkTypeRanges(vec![LinkTypeRange::Empty])),
    );
    get_links(
        base(),
        LinkZomes::range(
            LinkZomes::A(integrity_a::LinkTypes::A)..LinkZomes::A(integrity_a::LinkTypes::A),
        ),
        None,
    )
    .unwrap();

    // Include types in this vec.
    set_zome_types_and_compare(
        vec![],
        vec![12..15, 5..8],
        GetLinks(make_range(vec![12..=12, 5..=5, 6..=6, 13..=13])),
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
        vec![],
        vec![12..15, 5..8],
        GetLinks(make_range(vec![5..=5, 14..=14])),
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
        vec![],
        vec![12..15, 5..8],
        GetLinks(make_range(vec![5..=5, 14..=14])),
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
        vec![],
        vec![12..15, 5..8],
        GetLinks(make_range(vec![12..=12, 14..=14])),
    );
    get_links(base(), &t[..], None).unwrap();

    // Include all link types defined in any zome.
    set_zome_types_and_compare(
        vec![],
        vec![12..15, 5..8],
        GetLinks(LinkTypeRanges(vec![LinkTypeRange::Full])),
    );
    get_links(base(), .., None).unwrap();
}

#[test]
fn link_zomes_from_action() {
    use integrity_zomes::*;
    set_zome_types(vec![], vec![50..53, 100..103]);
    assert_eq!(
        LinkZomes::try_from(LocalZomeTypeId(0)),
        Ok(LinkZomes::A(integrity_a::LinkTypes::A))
    );
    assert_eq!(
        LinkZomes::try_from(&LocalZomeTypeId(0)),
        Ok(LinkZomes::A(integrity_a::LinkTypes::A))
    );
    assert_eq!(
        LinkZomes::try_from(LocalZomeTypeId(1)),
        Ok(LinkZomes::A(integrity_a::LinkTypes::B))
    );
    assert_eq!(
        LinkZomes::try_from(LocalZomeTypeId(2)),
        Ok(LinkZomes::A(integrity_a::LinkTypes::C))
    );
    assert_eq!(
        LinkZomes::try_from(LocalZomeTypeId(3)),
        Ok(LinkZomes::B(integrity_b::LinkTypes::A))
    );
    assert_eq!(
        LinkZomes::try_from(LocalZomeTypeId(4)),
        Ok(LinkZomes::B(integrity_b::LinkTypes::B))
    );
    assert_eq!(
        LinkZomes::try_from(LocalZomeTypeId(5)),
        Ok(LinkZomes::B(integrity_b::LinkTypes::C))
    );

    assert!(matches!(LinkZomes::try_from(LocalZomeTypeId(50)), Err(_)));

    assert_eq!(
        LinkZomes::try_from(LinkType(50)),
        Ok(LinkZomes::A(integrity_a::LinkTypes::A))
    );
    assert_eq!(
        LinkZomes::try_from(LinkType(51)),
        Ok(LinkZomes::A(integrity_a::LinkTypes::B))
    );
    assert_eq!(
        LinkZomes::try_from(LinkType(52)),
        Ok(LinkZomes::A(integrity_a::LinkTypes::C))
    );
    assert_eq!(
        LinkZomes::try_from(LinkType(100)),
        Ok(LinkZomes::B(integrity_b::LinkTypes::A))
    );
    assert_eq!(
        LinkZomes::try_from(LinkType(101)),
        Ok(LinkZomes::B(integrity_b::LinkTypes::B))
    );
    assert_eq!(
        LinkZomes::try_from(LinkType(102)),
        Ok(LinkZomes::B(integrity_b::LinkTypes::C))
    );

    assert!(matches!(LinkZomes::try_from(LinkType(0)), Err(_)));
}

enum Compare {
    GetLinks(LinkTypeRanges),
    CreateLink(LinkType),
    Nothing,
}

fn make_range(r: Vec<std::ops::RangeInclusive<u8>>) -> LinkTypeRanges {
    LinkTypeRanges(
        r.into_iter()
            .map(|r| LinkTypeRange::Inclusive(LinkType(*r.start())..=LinkType(*r.end())))
            .collect(),
    )
}

fn base() -> AnyLinkableHash {
    AnyLinkableHash::from_raw_36_and_type(vec![0; 36], AnyLinkable::External)
}

fn set_zome_types(entries: Vec<Range<u8>>, links: Vec<Range<u8>>) {
    set_zome_types_and_compare(entries, links, Compare::Nothing)
}
fn set_zome_types_and_compare(entries: Vec<Range<u8>>, links: Vec<Range<u8>>, compare: Compare) {
    let mut mock_hdk = MockHdkT::new();
    mock_hdk.expect_zome_info().returning(move |_| {
        let zome_types = ScopedZomeTypesSet {
            entries: ScopedZomeTypes(
                entries
                    .clone()
                    .into_iter()
                    .map(|r| GlobalZomeTypeId(r.start)..GlobalZomeTypeId(r.end))
                    .collect(),
            ),
            links: ScopedZomeTypes(
                links
                    .clone()
                    .into_iter()
                    .map(|r| GlobalZomeTypeId(r.start)..GlobalZomeTypeId(r.end))
                    .collect(),
            ),
            rate_limits: Default::default(),
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
    if !matches!(compare, Compare::CreateLink(_)) {
        mock_hdk
            .expect_create_link()
            .returning(|_| Ok(ActionHash::from_raw_36(vec![0u8; 36])));
    }
    if !matches!(compare, Compare::GetLinks(_)) {
        mock_hdk.expect_get_links().returning(|_| {
            Ok(vec![vec![Link {
                target: base(),
                timestamp: Timestamp(0),
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
                        tag: ().into(),
                        create_link_hash: ActionHash::from_raw_36(vec![0u8; 36]),
                    }]])
                });
        }
        Compare::CreateLink(l) => {
            mock_hdk
                .expect_create_link()
                .withf(move |CreateLinkInput { link_type, .. }| *link_type == l)
                .returning(|_| Ok(ActionHash::from_raw_36(vec![0u8; 36])));
        }
        Compare::Nothing => (),
    }
    set_hdk(mock_hdk);
}
