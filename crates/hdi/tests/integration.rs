#![cfg(feature = "test_utils")]
//! Tests for the proc macros defined in [`hdk_derive`] that are
//! used at the integrity level.

use hdi::prelude::*;
use hdi::test_utils::set_zome_types;

mod op;

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
        #[entry_def(required_validations = 10, cache_at_agent_activity = true)]
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
                ..Default::default()
            },
            EntryDef {
                id: "b".into(),
                visibility: EntryVisibility::Private,
                required_validations: Default::default(),
                ..Default::default()
            },
            EntryDef {
                id: "c".into(),
                visibility: Default::default(),
                required_validations: RequiredValidations(10),
                cache_at_agent_activity: true,
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
                ..Default::default()
            },
            EntryDef {
                id: "b".into(),
                visibility: Default::default(),
                required_validations: Default::default(),
                ..Default::default()
            },
            EntryDef {
                id: "c".into(),
                visibility: Default::default(),
                required_validations: Default::default(),
                ..Default::default()
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
        Err(_)
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
