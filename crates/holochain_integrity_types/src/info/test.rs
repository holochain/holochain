use super::*;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum LinkTypes {
    A,
    B,
}
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum LinkZomes {
    A(LinkTypes),
    B(LinkTypes),
}

fn make_map(ids: &[(u8, u8)]) -> ScopedZomeTypes<LinkType> {
    ScopedZomeTypes(
        ids.into_iter()
            .map(|(zome_id, len)| ((*zome_id).into(), (0..*len).map(|t| t.into()).collect()))
            .collect(),
    )
}

fn make_entry_map(ids: &[(u8, u8)]) -> ScopedZomeTypes<EntryDefIndex> {
    ScopedZomeTypes(
        ids.into_iter()
            .map(|(zome_id, len)| ((*zome_id).into(), (0..*len).map(|t| t.into()).collect()))
            .collect(),
    )
}

impl From<LinkTypes> for ZomeLinkTypesKey {
    fn from(lt: LinkTypes) -> Self {
        match lt {
            LinkTypes::A => ZomeTypesKey {
                zome_index: 0.into(),
                type_index: 0.into(),
            },
            LinkTypes::B => ZomeTypesKey {
                zome_index: 0.into(),
                type_index: 1.into(),
            },
        }
    }
}

impl From<LinkZomes> for ZomeLinkTypesKey {
    fn from(lt: LinkZomes) -> Self {
        match lt {
            LinkZomes::A(lt) => ZomeTypesKey {
                zome_index: 0.into(),
                type_index: ZomeLinkTypesKey::from(lt).type_index,
            },
            LinkZomes::B(lt) => ZomeTypesKey {
                zome_index: 1.into(),
                type_index: ZomeLinkTypesKey::from(lt).type_index,
            },
        }
    }
}

impl LinkTypes {
    fn iter() -> impl Iterator<Item = Self> {
        use LinkTypes::*;
        [A, B].into_iter()
    }
}

impl LinkZomes {
    fn iter() -> impl Iterator<Item = Self> {
        use LinkZomes::*;
        LinkTypes::iter().map(A).chain(LinkTypes::iter().map(B))
    }
}

#[test]
fn can_map_to_key() {
    let map = make_map(&[(12, 2)]);
    assert_eq!(
        map.get(LinkTypes::A).unwrap(),
        ScopedLinkType {
            zome_id: 12.into(),
            zome_type: 0.into()
        }
    );
    assert_eq!(
        map.get(LinkTypes::B).unwrap(),
        ScopedLinkType {
            zome_id: 12.into(),
            zome_type: 1.into()
        }
    );

    let map = make_map(&[(12, 2), (3, 2)]);
    assert_eq!(
        map.get(LinkZomes::A(LinkTypes::A)).unwrap(),
        ScopedLinkType {
            zome_id: 12.into(),
            zome_type: 0.into()
        }
    );
    assert_eq!(
        map.get(LinkZomes::A(LinkTypes::B)).unwrap(),
        ScopedLinkType {
            zome_id: 12.into(),
            zome_type: 1.into()
        }
    );
    assert_eq!(
        map.get(LinkZomes::B(LinkTypes::A)).unwrap(),
        ScopedLinkType {
            zome_id: 3.into(),
            zome_type: 0.into()
        }
    );
    assert_eq!(
        map.get(LinkZomes::B(LinkTypes::B)).unwrap(),
        ScopedLinkType {
            zome_id: 3.into(),
            zome_type: 1.into()
        }
    );
}

#[test]
fn can_map_from_scoped_type() {
    let map = make_map(&[(12, 2)]);
    assert_eq!(
        map.find(
            LinkTypes::iter(),
            ScopedLinkType {
                zome_id: 12.into(),
                zome_type: 0.into()
            }
        )
        .unwrap(),
        LinkTypes::A
    );
    assert_eq!(
        map.find(
            LinkTypes::iter(),
            ScopedLinkType {
                zome_id: 12.into(),
                zome_type: 1.into()
            }
        )
        .unwrap(),
        LinkTypes::B
    );
    assert_eq!(
        map.find(
            LinkTypes::iter(),
            ScopedLinkType {
                zome_id: 12.into(),
                zome_type: 3.into()
            }
        ),
        None
    );
    assert_eq!(
        map.find(
            LinkTypes::iter(),
            ScopedLinkType {
                zome_id: 13.into(),
                zome_type: 1.into()
            }
        ),
        None
    );

    let map = make_map(&[(12, 2), (3, 2)]);
    assert_eq!(
        map.find(
            LinkZomes::iter(),
            ScopedLinkType {
                zome_id: 12.into(),
                zome_type: 0.into()
            }
        )
        .unwrap(),
        LinkZomes::A(LinkTypes::A),
    );
    assert_eq!(
        map.find(
            LinkZomes::iter(),
            ScopedLinkType {
                zome_id: 12.into(),
                zome_type: 1.into()
            }
        )
        .unwrap(),
        LinkZomes::A(LinkTypes::B),
    );
    assert_eq!(
        map.find(
            LinkZomes::iter(),
            ScopedLinkType {
                zome_id: 3.into(),
                zome_type: 0.into()
            }
        )
        .unwrap(),
        LinkZomes::B(LinkTypes::A),
    );
    assert_eq!(
        map.find(
            LinkZomes::iter(),
            ScopedLinkType {
                zome_id: 3.into(),
                zome_type: 1.into()
            }
        )
        .unwrap(),
        LinkZomes::B(LinkTypes::B),
    );
    assert_eq!(
        map.find(
            LinkZomes::iter(),
            ScopedLinkType {
                zome_id: 3.into(),
                zome_type: 2.into()
            }
        ),
        None
    );
    assert_eq!(
        map.find(
            LinkZomes::iter(),
            ScopedLinkType {
                zome_id: 12.into(),
                zome_type: 2.into()
            }
        ),
        None
    );
    assert_eq!(
        map.find(
            LinkZomes::iter(),
            ScopedLinkType {
                zome_id: 14.into(),
                zome_type: 0.into()
            }
        ),
        None
    );
}

type Entry = ();
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
struct A;
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
struct B;
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum EntryTypes {
    A(A),
    B(B),
}
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum EntryZomes {
    A(EntryTypes),
    B(EntryTypes),
}

impl crate::UnitEnum for EntryTypes {
    type Unit = UnitEntry;

    fn to_unit(&self) -> Self::Unit {
        match self {
            EntryTypes::A(_) => Self::Unit::A,
            EntryTypes::B(_) => Self::Unit::B,
        }
    }
}
enum UnitEntry {
    A,
    B,
}

impl From<Entry> for A {
    fn from(_: Entry) -> Self {
        A {}
    }
}

impl From<Entry> for B {
    fn from(_: Entry) -> Self {
        B {}
    }
}

impl From<(ZomeEntryTypesKey, Entry)> for EntryTypes {
    fn from((k, entry): (ZomeEntryTypesKey, Entry)) -> Self {
        match k {
            ZomeTypesKey {
                zome_index: ZomeDependencyIndex(0),
                type_index: EntryDefIndex(0),
            } => EntryTypes::A(entry.into()),
            ZomeTypesKey {
                zome_index: ZomeDependencyIndex(0),
                type_index: EntryDefIndex(1),
            } => EntryTypes::B(entry.into()),
            _ => unreachable!(),
        }
    }
}

impl From<ZomeEntryTypesKey> for UnitEntry {
    fn from(k: ZomeEntryTypesKey) -> Self {
        match k {
            ZomeTypesKey {
                zome_index: ZomeDependencyIndex(0),
                type_index: EntryDefIndex(0),
            } => Self::A,
            ZomeTypesKey {
                zome_index: ZomeDependencyIndex(0),
                type_index: EntryDefIndex(1),
            } => Self::B,
            _ => unreachable!(),
        }
    }
}
impl From<(ZomeEntryTypesKey, Entry)> for EntryZomes {
    fn from((k, entry): (ZomeEntryTypesKey, Entry)) -> Self {
        match k {
            ZomeTypesKey {
                zome_index: ZomeDependencyIndex(0),
                type_index,
            } => {
                let k = ZomeTypesKey {
                    zome_index: 0.into(),
                    type_index,
                };
                let unit: <EntryTypes as crate::UnitEnum>::Unit = k.into();
                let r = match unit {
                    UnitEntry::A => EntryTypes::A(entry.into()),
                    UnitEntry::B => EntryTypes::B(entry.into()),
                };
                Self::A(r)
            }
            ZomeTypesKey {
                zome_index: ZomeDependencyIndex(1),
                type_index,
            } => {
                let k = ZomeTypesKey {
                    zome_index: 0.into(),
                    type_index,
                };
                let unit: <EntryTypes as crate::UnitEnum>::Unit = k.into();
                let r = match unit {
                    UnitEntry::A => EntryTypes::A(entry.into()),
                    UnitEntry::B => EntryTypes::B(entry.into()),
                };
                Self::B(r)
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn can_map_entry_from_scoped_type() {
    let map = make_entry_map(&[(12, 2), (34, 2)]);
    let find_key = |zome_id: u8, zome_type: u8| {
        let input = ScopedEntryDefIndex {
            zome_id: zome_id.into(),
            zome_type: zome_type.into(),
        };

        map.find_key(input).unwrap()
    };
    assert_eq!(
        find_key(12, 0),
        ZomeEntryTypesKey {
            zome_index: 0.into(),
            type_index: 0.into()
        }
    );
    assert_eq!(EntryTypes::from((find_key(12, 0), ())), EntryTypes::A(A {}));
    assert_eq!(EntryTypes::from((find_key(12, 1), ())), EntryTypes::B(B {}));

    assert_eq!(
        EntryZomes::from((find_key(12, 0), ())),
        EntryZomes::A(EntryTypes::A(A {}))
    );
    assert_eq!(
        EntryZomes::from((find_key(12, 1), ())),
        EntryZomes::A(EntryTypes::B(B {}))
    );
    assert_eq!(
        EntryZomes::from((find_key(34, 0), ())),
        EntryZomes::B(EntryTypes::A(A {}))
    );
    assert_eq!(
        EntryZomes::from((find_key(34, 1), ())),
        EntryZomes::B(EntryTypes::B(B {}))
    );
}
