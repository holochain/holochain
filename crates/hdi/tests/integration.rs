//! Tests for the proc macros defined in [`hdk_derive`] that are
//! used at the integrity level.

use arbitrary::Arbitrary;
use arbitrary::Unstructured;
use hdi::prelude::*;
use test_case::test_case;

fn to_coords(t: impl Into<ZomeLinkTypesKey>) -> (u8, u8) {
    let t = t.into();
    (t.zome_index.0, t.type_index.0)
}

fn zome_and_link_type<T>(t: T) -> (u8, u8)
where
    T: Copy,
    ScopedLinkType: TryFrom<T, Error = WasmError>,
{
    let t: ScopedLinkType = t.try_into().unwrap();
    (t.zome_id.0, t.zome_type.0)
}

fn scoped_link_type(zome_id: u8, zome_type: u8) -> ScopedLinkType {
    ScopedLinkType {
        zome_id: zome_id.into(),
        zome_type: zome_type.into(),
    }
}

fn zome_and_entry_type<T>(t: T) -> (u8, u8)
where
    ScopedEntryDefIndex: TryFrom<T, Error = WasmError>,
{
    let t: ScopedEntryDefIndex = t.try_into().unwrap();
    (t.zome_id.0, t.zome_type.0)
}

#[test]
fn to_local_types_test_unit() {
    #[hdk_to_coordinates]
    enum Unit {
        A,
        B,
        C,
    }

    assert_eq!(to_coords(Unit::A), (0, 0));
    assert_eq!(to_coords(&Unit::A), (0, 0));
    assert_eq!(to_coords(Unit::B), (0, 1));
    assert_eq!(to_coords(Unit::C), (0, 2));
}

#[test]
/// Setting the discriminant explicitly should have no effect.
fn to_local_types_test_discriminant() {
    #[hdk_to_coordinates]
    enum Unit {
        A = 12,
        B = 3000,
        C = 1,
    }

    assert_eq!(to_coords(Unit::A), (0, 0));
    assert_eq!(to_coords(&Unit::A), (0, 0));
    assert_eq!(to_coords(Unit::B), (0, 1));
    assert_eq!(to_coords(Unit::C), (0, 2));
}

#[test]
fn to_local_types_test_nested() {
    #[hdk_to_coordinates]
    enum Nested1 {
        A,
        B,
    }

    #[hdk_to_coordinates]
    enum Nested2 {
        X,
        Y,
        Z,
    }

    #[hdk_to_coordinates]
    enum NoNesting {
        A(Nested1),
        #[allow(dead_code)]
        B {
            nested: Nested2,
        },
        C,
    }

    assert_eq!(to_coords(NoNesting::A(Nested1::A)), (0, 0));
    assert_eq!(to_coords(NoNesting::A(Nested1::B)), (0, 0));
    assert_eq!(to_coords(&NoNesting::A(Nested1::B)), (0, 0));
    assert_eq!(to_coords(NoNesting::B { nested: Nested2::X }), (0, 1));
    assert_eq!(to_coords(NoNesting::B { nested: Nested2::Y }), (0, 1));
    assert_eq!(to_coords(NoNesting::B { nested: Nested2::Z }), (0, 1));
    assert_eq!(to_coords(NoNesting::C), (0, 2));

    #[hdk_to_coordinates(nested)]
    enum Nesting {
        A(Nested1),
        #[allow(dead_code)]
        B {
            nested: Nested2,
        },
        C,
        D(Nested2),
    }

    assert_eq!(to_coords(Nesting::A(Nested1::A)), (0, 0));
    assert_eq!(to_coords(Nesting::A(Nested1::B)), (0, 1));
    assert_eq!(to_coords(&Nesting::A(Nested1::B)), (0, 1));
    assert_eq!(to_coords(Nesting::B { nested: Nested2::X }), (1, 0));
    assert_eq!(to_coords(Nesting::B { nested: Nested2::Y }), (1, 1));
    assert_eq!(to_coords(Nesting::B { nested: Nested2::Z }), (1, 2));
    assert_eq!(to_coords(Nesting::C), (2, 0));
    assert_eq!(to_coords(Nesting::D(Nested2::X)), (3, 0));
    assert_eq!(to_coords(Nesting::D(Nested2::Y)), (3, 1));
    assert_eq!(to_coords(Nesting::D(Nested2::Z)), (3, 2));

    assert_eq!(Nesting::ENUM_LEN, 9);
}

#[test]
fn to_zome_id_test_unit() {
    mod integrity_a {
        use super::*;
        #[hdk_link_types(skip_no_mangle = true)]
        pub enum Unit {
            A,
            B,
            C,
        }
    }

    mod integrity_b {
        use super::*;
        #[hdk_link_types(skip_no_mangle = true)]
        pub enum Unit {
            A,
            B,
            C,
        }
    }

    set_zome_types(&[], &[(0, 3)]);

    assert_eq!(zome_and_link_type(integrity_a::Unit::A), (0, 0));
    assert_eq!(zome_and_link_type(&integrity_a::Unit::A), (0, 0));
    assert_eq!(zome_and_link_type(integrity_a::Unit::B), (0, 1));
    assert_eq!(zome_and_link_type(integrity_a::Unit::C), (0, 2));

    set_zome_types(&[], &[(1, 3)]);

    assert_eq!(zome_and_link_type(integrity_b::Unit::A), (1, 0));
    assert_eq!(zome_and_link_type(&integrity_b::Unit::A), (1, 0));
    assert_eq!(zome_and_link_type(integrity_b::Unit::B), (1, 1));
    assert_eq!(zome_and_link_type(integrity_b::Unit::C), (1, 2));
}

mod entry_defs_to_entry_type_index_test {
    use hdi::prelude::*;

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
    }
}
mod entry_defs_overrides_mod {
    use super::*;
    #[hdk_entry_helper]
    pub struct A;
    #[hdk_entry_defs(skip_hdk_extern = true)]
    #[unit_enum(UnitFoo)]
    pub enum EntryTypes {
        #[entry_def(name = "hey")]
        A(A),
        #[entry_def(visibility = "private")]
        B(A),
        #[entry_def(required_validations = 10)]
        C(A),
    }
}

#[test]
fn entry_defs_overrides() {
    assert_eq!(
        entry_defs_overrides_mod::entry_defs(()).unwrap(),
        EntryDefsCallbackResult::Defs(EntryDefs(vec![
            EntryDef {
                id: "hey".into(),
                visibility: Default::default(),
                required_validations: Default::default(),
            },
            EntryDef {
                id: "b".into(),
                visibility: EntryVisibility::Private,
                required_validations: Default::default(),
            },
            EntryDef {
                id: "c".into(),
                visibility: Default::default(),
                required_validations: RequiredValidations(10),
            },
        ]))
    );
}

mod entry_defs_default_mod {
    use super::*;
    #[hdk_entry_helper]
    pub struct A;
    #[hdk_entry_defs(skip_hdk_extern = true)]
    #[unit_enum(UnitFoo2)]
    pub enum EntryTypes {
        A(A),
        B(A),
        C(A),
    }
}

#[test]
fn entry_defs_default() {
    assert_eq!(
        entry_defs_default_mod::entry_defs(()).unwrap(),
        EntryDefsCallbackResult::Defs(EntryDefs(vec![
            EntryDef {
                id: "a".into(),
                visibility: Default::default(),
                required_validations: Default::default(),
            },
            EntryDef {
                id: "b".into(),
                visibility: Default::default(),
                required_validations: Default::default(),
            },
            EntryDef {
                id: "c".into(),
                visibility: Default::default(),
                required_validations: Default::default(),
            },
        ]))
    );
}

#[test]
fn entry_defs_to_entry_type_index() {
    use entry_defs_to_entry_type_index_test::*;

    // Set the integrity_a scope.
    set_zome_types(&[(1, 3)], &[]);

    assert_eq!(
        zome_and_entry_type(integrity_a::EntryTypes::A(A {})),
        (1, 0)
    );
    assert_eq!(
        zome_and_entry_type(&integrity_a::EntryTypes::A(A {})),
        (1, 0)
    );
    assert_eq!(
        zome_and_entry_type(integrity_a::EntryTypes::B(B {})),
        (1, 1)
    );
    assert_eq!(
        zome_and_entry_type(integrity_a::EntryTypes::C(C {})),
        (1, 2)
    );

    assert!(matches!(
        integrity_a::EntryTypes::deserialize_from_type(1, 0, &Entry::try_from(A {}).unwrap()),
        Ok(Some(integrity_a::EntryTypes::A(A {})))
    ));
    assert!(matches!(
        integrity_a::EntryTypes::deserialize_from_type(1, 1, &Entry::try_from(A {}).unwrap()),
        Ok(Some(integrity_a::EntryTypes::B(B {})))
    ));
    assert!(matches!(
        integrity_a::EntryTypes::deserialize_from_type(1, 2, &Entry::try_from(A {}).unwrap()),
        Ok(Some(integrity_a::EntryTypes::C(C {})))
    ));

    assert!(matches!(
        integrity_a::EntryTypes::deserialize_from_type(1, 20, &Entry::try_from(A {}).unwrap()),
        Ok(None)
    ));
    assert!(matches!(
        integrity_a::EntryTypes::deserialize_from_type(0, 0, &Entry::try_from(A {}).unwrap()),
        Ok(None)
    ));

    // Set the integrity_b scope.
    set_zome_types(&[(12, 3)], &[]);

    assert_eq!(
        zome_and_entry_type(integrity_b::EntryTypes::A(A {})),
        (12, 0)
    );
    assert_eq!(
        zome_and_entry_type(&integrity_b::EntryTypes::A(A {})),
        (12, 0)
    );
    assert_eq!(
        zome_and_entry_type(integrity_b::EntryTypes::B(B {})),
        (12, 1)
    );
    assert_eq!(
        zome_and_entry_type(integrity_b::EntryTypes::C(C {})),
        (12, 2)
    );

    assert!(matches!(
        integrity_b::EntryTypes::deserialize_from_type(12, 0, &Entry::try_from(A {}).unwrap()),
        Ok(Some(integrity_b::EntryTypes::A(A {})))
    ));
    assert!(matches!(
        integrity_b::EntryTypes::deserialize_from_type(12, 1, &Entry::try_from(A {}).unwrap()),
        Ok(Some(integrity_b::EntryTypes::B(B {})))
    ));
    assert!(matches!(
        integrity_b::EntryTypes::deserialize_from_type(12, 2, &Entry::try_from(A {}).unwrap()),
        Ok(Some(integrity_b::EntryTypes::C(C {})))
    ));

    assert!(matches!(
        integrity_b::EntryTypes::deserialize_from_type(0, 20, &Entry::try_from(A {}).unwrap()),
        Ok(None)
    ));
    assert!(matches!(
        integrity_b::EntryTypes::deserialize_from_type(0, 0, &Entry::try_from(A {}).unwrap()),
        Ok(None)
    ));
}

#[test]
fn link_types_from_action() {
    #[hdk_link_types(skip_no_mangle = true)]
    pub enum LinkTypes {
        A,
        B,
        C,
    }
    set_zome_types(&[], &[(1, 3)]);
    assert_eq!(
        LinkTypes::try_from(scoped_link_type(1, 0)),
        Ok(LinkTypes::A)
    );
    assert_eq!(
        LinkTypes::try_from(scoped_link_type(1, 1)),
        Ok(LinkTypes::B)
    );
    assert_eq!(
        LinkTypes::try_from(scoped_link_type(1, 2)),
        Ok(LinkTypes::C)
    );
    assert!(matches!(
        LinkTypes::try_from(scoped_link_type(1, 50)),
        Err(_)
    ));
    assert!(matches!(
        LinkTypes::try_from(scoped_link_type(0, 1)),
        Err(_)
    ));
}

#[test]
fn link_types_to_global() {
    #[hdk_link_types(skip_no_mangle = true)]
    pub enum LinkTypes {
        A,
        B,
        C,
    }

    assert_eq!(__num_link_types(), 3);
}

mod op_type {
    use super::*;
    #[hdk_entry_helper]
    #[derive(Clone, PartialEq, Eq)]
    pub struct A;
    #[hdk_entry_helper]
    #[derive(Clone, PartialEq, Eq)]
    pub struct B;
    #[hdk_entry_helper]
    #[derive(Clone, PartialEq, Eq)]
    pub struct C;

    #[hdk_entry_defs(skip_hdk_extern = true)]
    #[unit_enum(UnitEntryTypes)]
    #[derive(Clone, PartialEq, Eq)]
    pub enum EntryTypes {
        A(A),
        #[entry_def(visibility = "private")]
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

fn eh(i: u8) -> EntryHash {
    EntryHash::from_raw_36(vec![i; 36])
}

fn ah(i: u8) -> ActionHash {
    ActionHash::from_raw_36(vec![i; 36])
}

fn ak(i: u8) -> AgentPubKey {
    AgentPubKey::from_raw_36(vec![i; 36])
}

fn lh(i: u8) -> AnyLinkableHash {
    AnyLinkableHash::from(EntryHash::from_raw_36(vec![i; 36]))
}

// Register Agent Activity
// Store Record
// Entries
#[test_case(OpType::StoreRecord(OpRecord::CreateEntry {entry_hash: eh(0), entry_type: op_type::EntryTypes::A(op_type::A{}) }))]
#[test_case(OpType::StoreRecord(OpRecord::UpdateEntry {entry_hash: eh(0), original_action_hash: ah(1), original_entry_hash: eh(1), entry_type: op_type::EntryTypes::A(op_type::A{}) }))]
#[test_case(OpType::StoreRecord(OpRecord::CreateEntry {entry_hash: eh(0), entry_type: op_type::EntryTypes::C(op_type::C{}) }))]
#[test_case(OpType::StoreRecord(OpRecord::UpdateEntry {entry_hash: eh(0), original_action_hash: ah(1), original_entry_hash: eh(1), entry_type: op_type::EntryTypes::C(op_type::C{}) }))]
#[test_case(OpType::StoreRecord(OpRecord::CreateAgent(ak(4))))]
#[test_case(OpType::StoreRecord(OpRecord::UpdateAgent { original_key: ak(4), new_key: ak(8), original_action_hash: ah(2) }))]
#[test_case(OpType::StoreRecord(OpRecord::CreatePrivateEntry {entry_hash: eh(0), entry_type: op_type::UnitEntryTypes::A }))]
#[test_case(OpType::StoreRecord(OpRecord::UpdatePrivateEntry {entry_hash: eh(0), original_action_hash: ah(1), original_entry_hash: eh(1), entry_type: op_type::UnitEntryTypes::A }))]
// Links
#[test_case(OpType::StoreRecord(OpRecord::CreateLink {base_address: lh(0), target_address: lh(2), tag: ().into(), link_type: op_type::LinkTypes::A }))]
#[test_case(OpType::StoreRecord(OpRecord::DeleteLink(ah(4))))]
// Store Entry
#[test_case(OpType::StoreEntry(OpEntry::CreateEntry {entry_hash: eh(0), entry_type: op_type::EntryTypes::A(op_type::A{}) }))]
#[test_case(OpType::StoreEntry(OpEntry::UpdateEntry {entry_hash: eh(0), original_action_hash: ah(1), original_entry_hash: eh(1), entry_type: op_type::EntryTypes::A(op_type::A{}) }))]
#[test_case(OpType::StoreEntry(OpEntry::CreateAgent(ak(4))))]
#[test_case(OpType::StoreEntry(OpEntry::UpdateAgent { original_key: ak(4), new_key: ak(8), original_action_hash: ah(2) }))]
// Error Cases
// #[test_case(OpType::StoreEntry(OpEntry::CreateEntry {entry_hash: eh(0), entry_type: op_type::EntryTypes::B(op_type::B{}) }))]
// Register Update
#[test_case(OpType::RegisterUpdate(OpUpdate::Entry {entry_hash: eh(0), original_action_hash: ah(1), original_entry_hash: eh(1), new_entry_type: op_type::EntryTypes::A(op_type::A{}), original_entry_type: op_type::EntryTypes::A(op_type::A{}) }))]
#[test_case(OpType::RegisterUpdate(OpUpdate::Agent { original_key: ak(4), new_key: ak(8), original_action_hash: ah(2) }))]
// Register Delete
// Register Create Link
#[test_case(OpType::RegisterCreateLink {base_address: lh(0), target_address: lh(2), tag: ().into(), link_type: op_type::LinkTypes::A })]
#[test_case(OpType::RegisterCreateLink {base_address: lh(0), target_address: lh(2), tag: ().into(), link_type: op_type::LinkTypes::B })]
// Register Delete Link
#[test_case(OpType::RegisterDeleteLink {original_link_hash: ah(2), base_address: lh(0), target_address: lh(2), tag: ().into(), link_type: op_type::LinkTypes::A })]
#[test_case(OpType::RegisterDeleteLink {original_link_hash: ah(2), base_address: lh(0), target_address: lh(2), tag: ().into(), link_type: op_type::LinkTypes::C })]
fn op_into_type(op: OpType<op_type::EntryTypes, op_type::LinkTypes>) {
    set_zome_types(&[(0, 3)], &[(0, 3)]);
    let data = vec![0u8; 2000];
    let mut ud = Unstructured::new(&data);
    let o = match op.clone() {
        OpType::StoreRecord(OpRecord::CreateEntry {
            entry_hash,
            entry_type: et,
        }) => {
            let entry = RecordEntry::Present(Entry::try_from(&et).unwrap());
            let t = ScopedEntryDefIndex::try_from(&et).unwrap();
            let c = create((&et).into(), &mut ud, t, entry_hash);
            let c = Action::Create(c);
            store_record_entry(c, entry)
        }
        OpType::StoreRecord(OpRecord::CreatePrivateEntry {
            entry_hash,
            entry_type: et,
        }) => {
            let t = ScopedEntryDefIndex::try_from(&et).unwrap();
            let c = create(EntryVisibility::Private, &mut ud, t, entry_hash);
            let c = Action::Create(c);
            store_record_entry(c, RecordEntry::Hidden)
        }
        OpType::StoreRecord(OpRecord::CreateAgent(agent)) => {
            let entry = RecordEntry::Present(Entry::Agent(agent.clone()));
            let mut c = Create::arbitrary(&mut ud).unwrap();
            c.entry_type = EntryType::AgentPubKey;
            c.entry_hash = agent.into();
            let c = Action::Create(c);
            store_record_entry(c, entry)
        }
        OpType::StoreRecord(OpRecord::CreateLink {
            link_type: lt,
            base_address,
            target_address,
            tag,
        }) => {
            let t = ScopedLinkType::try_from(&lt).unwrap();
            let mut c = CreateLink::arbitrary(&mut ud).unwrap();
            c.zome_id = t.zome_id;
            c.link_type = t.zome_type;
            c.base_address = base_address;
            c.target_address = target_address;
            c.tag = tag;
            let c = Action::CreateLink(c);
            Op::StoreRecord(StoreRecord {
                record: Record {
                    signed_action: SignedHashed {
                        hashed: ActionHashed::from_content_sync(c),
                        signature: Signature::arbitrary(&mut ud).unwrap(),
                    },
                    entry: RecordEntry::NotApplicable,
                },
            })
        }
        OpType::StoreRecord(OpRecord::DeleteLink(link_action_hash)) => {
            let mut c = DeleteLink::arbitrary(&mut ud).unwrap();
            c.link_add_address = link_action_hash;
            let c = Action::DeleteLink(c);
            Op::StoreRecord(StoreRecord {
                record: Record {
                    signed_action: SignedHashed {
                        hashed: ActionHashed::from_content_sync(c),
                        signature: Signature::arbitrary(&mut ud).unwrap(),
                    },
                    entry: RecordEntry::NotApplicable,
                },
            })
        }
        OpType::StoreRecord(OpRecord::UpdateEntry {
            entry_hash,
            original_action_hash,
            original_entry_hash,
            entry_type: et,
        }) => {
            let entry = RecordEntry::Present(Entry::try_from(&et).unwrap());
            let t = ScopedEntryDefIndex::try_from(&et).unwrap();
            let u = update(
                (&et).into(),
                &mut ud,
                t,
                entry_hash,
                original_action_hash,
                original_entry_hash,
            );
            let u = Action::Update(u);
            store_record_entry(u, entry)
        }
        OpType::StoreRecord(OpRecord::UpdatePrivateEntry {
            entry_hash,
            original_action_hash,
            original_entry_hash,
            entry_type: et,
        }) => {
            let t = ScopedEntryDefIndex::try_from(&et).unwrap();
            let u = update(
                EntryVisibility::Private,
                &mut ud,
                t,
                entry_hash,
                original_action_hash,
                original_entry_hash,
            );
            let u = Action::Update(u);
            store_record_entry(u, RecordEntry::Hidden)
        }
        OpType::StoreRecord(OpRecord::UpdateAgent {
            original_action_hash,
            original_key,
            new_key,
        }) => {
            let entry = RecordEntry::Present(Entry::Agent(new_key.clone()));
            let mut u = Update::arbitrary(&mut ud).unwrap();
            u.entry_type = EntryType::AgentPubKey;
            u.entry_hash = new_key.into();
            u.original_action_address = original_action_hash;
            u.original_entry_address = original_key.into();
            let u = Action::Update(u);
            store_record_entry(u, entry)
        }
        OpType::StoreEntry(OpEntry::CreateEntry {
            entry_hash,
            entry_type: et,
        }) => {
            let entry = Entry::try_from(&et).unwrap();
            let t = ScopedEntryDefIndex::try_from(&et).unwrap();
            let c = create(EntryVisibility::Public, &mut ud, t, entry_hash);
            let c = EntryCreationAction::Create(c);
            store_entry_entry(c, entry)
        }
        OpType::StoreEntry(OpEntry::UpdateEntry {
            entry_hash,
            original_action_hash,
            original_entry_hash,
            entry_type: et,
        }) => {
            let entry = Entry::try_from(&et).unwrap();
            let t = ScopedEntryDefIndex::try_from(&et).unwrap();
            let u = update(
                (&et).into(),
                &mut ud,
                t,
                entry_hash,
                original_action_hash,
                original_entry_hash,
            );
            let u = EntryCreationAction::Update(u);
            store_entry_entry(u, entry)
        }
        OpType::StoreEntry(OpEntry::CreateAgent(agent)) => {
            let entry = Entry::Agent(agent.clone());
            let mut c = Create::arbitrary(&mut ud).unwrap();
            c.entry_type = EntryType::AgentPubKey;
            c.entry_hash = agent.into();
            let c = EntryCreationAction::Create(c);
            store_entry_entry(c, entry)
        }
        OpType::StoreEntry(OpEntry::UpdateAgent {
            original_action_hash,
            original_key,
            new_key,
        }) => {
            let entry = Entry::Agent(new_key.clone());
            let mut u = Update::arbitrary(&mut ud).unwrap();
            u.entry_type = EntryType::AgentPubKey;
            u.entry_hash = new_key.into();
            u.original_action_address = original_action_hash;
            u.original_entry_address = original_key.into();
            let u = EntryCreationAction::Update(u);
            store_entry_entry(u, entry)
        }
        OpType::RegisterCreateLink {
            link_type: lt,
            base_address,
            target_address,
            tag,
        } => {
            let t = ScopedLinkType::try_from(&lt).unwrap();
            let mut c = CreateLink::arbitrary(&mut ud).unwrap();
            c.zome_id = t.zome_id;
            c.link_type = t.zome_type;
            c.base_address = base_address;
            c.target_address = target_address;
            c.tag = tag;
            Op::RegisterCreateLink(RegisterCreateLink {
                create_link: SignedHashed {
                    hashed: HoloHashed::from_content_sync(c),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
            })
        }
        OpType::RegisterDeleteLink {
            original_link_hash,
            link_type: lt,
            base_address,
            target_address,
            tag,
        } => {
            let t = ScopedLinkType::try_from(&lt).unwrap();
            let mut c = CreateLink::arbitrary(&mut ud).unwrap();
            let mut d = DeleteLink::arbitrary(&mut ud).unwrap();
            d.link_add_address = original_link_hash;
            c.zome_id = t.zome_id;
            c.link_type = t.zome_type;
            c.base_address = base_address;
            c.target_address = target_address;
            c.tag = tag;
            Op::RegisterDeleteLink(RegisterDeleteLink {
                delete_link: SignedHashed {
                    hashed: HoloHashed::from_content_sync(d),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                create_link: c,
            })
        }
        OpType::RegisterUpdate(OpUpdate::Entry {
            entry_hash,
            original_action_hash,
            original_entry_hash,
            original_entry_type: original_et,
            new_entry_type: et,
        }) => {
            let original_entry = Entry::try_from(&original_et).unwrap();
            let entry = Entry::try_from(&et).unwrap();
            let t = ScopedEntryDefIndex::try_from(&et).unwrap();
            let original_action = update(
                (&original_et).into(),
                &mut ud,
                t,
                entry_hash.clone(),
                original_action_hash.clone(),
                original_entry_hash.clone(),
            );
            let original_action = EntryCreationAction::Update(original_action);
            let u = update(
                (&et).into(),
                &mut ud,
                t,
                entry_hash,
                original_action_hash,
                original_entry_hash,
            );
            Op::RegisterUpdate(RegisterUpdate {
                update: SignedHashed {
                    hashed: HoloHashed::from_content_sync(u),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                new_entry: entry,
                original_action,
                original_entry,
            })
        }
        OpType::RegisterUpdate(OpUpdate::Agent {
            original_action_hash,
            original_key,
            new_key,
        }) => {
            let entry = Entry::Agent(new_key.clone());
            let original_entry = Entry::Agent(original_key.clone());
            let mut u = Update::arbitrary(&mut ud).unwrap();
            let c = Create::arbitrary(&mut ud).unwrap();
            u.entry_type = EntryType::AgentPubKey;
            u.entry_hash = new_key.into();
            u.original_action_address = original_action_hash;
            u.original_entry_address = original_key.into();
            let original_action = EntryCreationAction::Create(c);
            Op::RegisterUpdate(RegisterUpdate {
                update: SignedHashed {
                    hashed: HoloHashed::from_content_sync(u),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                new_entry: entry,
                original_action,
                original_entry,
            })
        }
    };
    assert_eq!(o.into_type().unwrap(), op);
}

fn store_record_entry(action: Action, entry: RecordEntry) -> Op {
    Op::StoreRecord(StoreRecord {
        record: Record {
            signed_action: SignedHashed {
                hashed: ActionHashed::from_content_sync(action),
                signature: Signature([0u8; 64]),
            },
            entry,
        },
    })
}
fn store_entry_entry(action: EntryCreationAction, entry: Entry) -> Op {
    Op::StoreEntry(StoreEntry {
        action: SignedHashed {
            hashed: HoloHashed::from_content_sync(action),
            signature: Signature([0u8; 64]),
        },
        entry,
    })
}

fn create(
    visibility: EntryVisibility,
    ud: &mut Unstructured,
    t: ScopedEntryDefIndex,
    entry_hash: EntryHash,
) -> Create {
    let mut c = Create::arbitrary(ud).unwrap();
    c.entry_type = EntryType::App(AppEntryType {
        id: t.zome_type,
        zome_id: t.zome_id,
        visibility,
    });
    c.entry_hash = entry_hash;
    c
}
fn update(
    visibility: EntryVisibility,
    ud: &mut Unstructured,
    t: ScopedEntryDefIndex,
    entry_hash: EntryHash,
    original_action_hash: ActionHash,
    original_entry_hash: EntryHash,
) -> Update {
    let mut u = Update::arbitrary(ud).unwrap();
    u.entry_type = EntryType::App(AppEntryType {
        id: t.zome_type,
        zome_id: t.zome_id,
        visibility,
    });
    u.entry_hash = entry_hash;
    u.original_action_address = original_action_hash;
    u.original_entry_address = original_entry_hash;
    u
}
// #[test]
// fn op_into_type() {
//     fn empty_create() -> Create {
//         Create {
//             author: AgentPubKey::from_raw_36(vec![0u8; 36]),
//             timestamp: Timestamp(0),
//             action_seq: 1,
//             prev_action: ActionHash::from_raw_36(vec![0u8; 36]),
//             entry_type: EntryType::App(AppEntryType {
//                 id: 0.into(),
//                 zome_id: 0.into(),
//                 visibility: EntryVisibility::Public,
//             }),
//             entry_hash: EntryHash::from_raw_36(vec![0u8; 36]),
//             weight: Default::default(),
//         }
//     }
//     let op = Op::StoreRecord(StoreRecord {
//         record: Record {
//             signed_action: SignedHashed {
//                 hashed: ActionHashed {
//                     content: Action::Create(Create {
//                         entry_type: EntryType::App(AppEntryType {
//                             id: 0.into(),
//                             zome_id: 0.into(),
//                             visibility: EntryVisibility::Public,
//                         }),
//                         ..empty_create()
//                     }),
//                     hash: ActionHash::from_raw_36(vec![1u8; 36]),
//                 },
//                 signature: Signature([0u8; 64]),
//             },
//             entry: RecordEntry::Present(EntryTypes::A(A {}).try_into().unwrap()),
//         },
//     });
//     eprintln!("{}", serde_yaml::to_string(&op).unwrap());
//     set_zome_types(&[(0, 3)], &[(0, 3)]);
// match op.as_type().unwrap() {
//     OpType::StoreRecord(OpRecord::CreateEntry {
//         entry_hash,
//         entry_type: EntryTypes::A(_),
//     }) => {
//         op.action().timestamp()
//     },
//     OpType::StoreRecord(OpRecord::CreatePrivateEntry {
//         entry_hash,
//         entry_type: UnitEntryTypes::A,
//     }) => {
//         op.action().timestamp()
//     },
//     OpType::StoreRecord(OpRecord::UpdateEntry {
//         entry_hash,
//         original_action_hash,
//         entry_type: EntryTypes::A(_),
//     }) => {
//         op.action().timestamp()
//         op.action().prev_action()
//     },
//     OpType::StoreRecord(OpRecord::CreateEntry(EntryTypes::B(_))) => (),
//     OpType::StoreRecord(OpRecord::CreateEntry(EntryTypes::C(_))) => (),
//     OpType::StoreRecord(OpRecord::CreateHiddenEntry) => (),
//     OpType::StoreRecord(OpRecord::CreateEntryNotStored) => (),
//     OpType::StoreRecord(OpRecord::AgentPubKey(_)) => (),
//     OpType::StoreRecord(OpRecord::CreateLink(LinkTypes::A)) => (),
//     OpType::StoreRecord(OpRecord::CreateLink(LinkTypes::B)) => (),
//     OpType::StoreRecord(OpRecord::CreateLink(LinkTypes::C)) => (),
//     OpType::Link(LinkTypes::A) => todo!(),
//     OpType::Link(LinkTypes::B) => todo!(),
//     OpType::Link(LinkTypes::C) => todo!(),
// }
//     match op.into_type::<_, ()>().unwrap() {
//         OpType::StoreRecord(OpRecord::CreateEntry(EntryTypes::A(_))) => (),
//         _ => (),
//     }
//     match op.into_type::<(), _>().unwrap() {
//         OpType::StoreRecord(OpRecord::CreateLink(LinkTypes::A)) => (),
//         _ => (),
//     }
//     match op.into_type::<(), ()>().unwrap() {
//         OpType::StoreRecord(_) => (),
//         _ => (),
//     }
// }

fn set_zome_types(entries: &[(u8, u8)], links: &[(u8, u8)]) {
    struct TestHdi(ScopedZomeTypesSet);
    #[allow(unused_variables)]
    impl HdiT for TestHdi {
        fn verify_signature(&self, verify_signature: VerifySignature) -> ExternResult<bool> {
            todo!()
        }

        fn hash(&self, hash_input: HashInput) -> ExternResult<HashOutput> {
            todo!()
        }

        fn must_get_entry(
            &self,
            must_get_entry_input: MustGetEntryInput,
        ) -> ExternResult<EntryHashed> {
            todo!()
        }

        fn must_get_action(
            &self,
            must_get_action_input: MustGetActionInput,
        ) -> ExternResult<SignedActionHashed> {
            todo!()
        }

        fn must_get_valid_record(
            &self,
            must_get_valid_record_input: MustGetValidRecordInput,
        ) -> ExternResult<Record> {
            todo!()
        }

        fn dna_info(&self, dna_info_input: ()) -> ExternResult<DnaInfo> {
            todo!()
        }

        fn zome_info(&self, zome_info_input: ()) -> ExternResult<ZomeInfo> {
            let info = ZomeInfo {
                name: String::default().into(),
                id: u8::default().into(),
                properties: Default::default(),
                entry_defs: EntryDefs(Default::default()),
                extern_fns: Default::default(),
                zome_types: self.0.clone(),
            };
            Ok(info)
        }

        fn x_salsa20_poly1305_decrypt(
            &self,
            x_salsa20_poly1305_decrypt: XSalsa20Poly1305Decrypt,
        ) -> ExternResult<Option<XSalsa20Poly1305Data>> {
            todo!()
        }

        fn x_25519_x_salsa20_poly1305_decrypt(
            &self,
            x_25519_x_salsa20_poly1305_decrypt: X25519XSalsa20Poly1305Decrypt,
        ) -> ExternResult<Option<XSalsa20Poly1305Data>> {
            todo!()
        }

        fn trace(&self, trace_msg: TraceMsg) -> ExternResult<()> {
            todo!()
        }
    }
    set_hdi(TestHdi(ScopedZomeTypesSet {
        entries: ScopedZomeTypes(
            entries
                .into_iter()
                .map(|(z, types)| (ZomeId(*z), (0..*types).map(|t| EntryDefIndex(t)).collect()))
                .collect(),
        ),
        links: ScopedZomeTypes(
            links
                .into_iter()
                .map(|(z, types)| (ZomeId(*z), (0..*types).map(|t| LinkType(t)).collect()))
                .collect(),
        ),
    }));
}
