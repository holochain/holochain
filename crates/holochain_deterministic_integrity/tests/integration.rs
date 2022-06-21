//! Tests for the proc macros defined in [`hdk_derive`] that are
//! used at the integrity level.
use std::ops::Range;

use holochain_deterministic_integrity::prelude::*;

fn local_type(t: impl Into<LocalZomeTypeId>) -> LocalZomeTypeId {
    t.into()
}

fn zome_and_link_type<T>(t: T) -> (ZomeId, LinkType)
where
    T: Copy,
    ZomeId: TryFrom<T, Error = WasmError>,
    LinkType: From<T>,
{
    (t.try_into().unwrap(), t.into())
}

fn zome_and_entry_type<T>(t: T) -> (ZomeId, EntryDefIndex)
where
    ZomeId: TryFrom<T, Error = WasmError>,
    EntryDefIndex: for<'a> From<&'a T>,
{
    let e = (&t).into();
    (t.try_into().unwrap(), e)
}

#[test]
fn to_local_types_test_unit() {
    #[hdk_to_local_types]
    enum Unit {
        A,
        B,
        C,
    }

    assert_eq!(local_type(Unit::A), LocalZomeTypeId(0u8));
    assert_eq!(local_type(&Unit::A), LocalZomeTypeId(0u8));
    assert_eq!(local_type(Unit::B), LocalZomeTypeId(1u8));
    assert_eq!(local_type(Unit::C), LocalZomeTypeId(2u8));
}

#[test]
/// Setting the discriminant explicitly should have no effect.
fn to_local_types_test_discriminant() {
    #[hdk_to_local_types]
    enum Unit {
        A = 12,
        B = 3000,
        C = 1,
    }

    assert_eq!(local_type(Unit::A), LocalZomeTypeId(0u8));
    assert_eq!(local_type(&Unit::A), LocalZomeTypeId(0u8));
    assert_eq!(local_type(Unit::B), LocalZomeTypeId(1u8));
    assert_eq!(local_type(Unit::C), LocalZomeTypeId(2u8));
}

#[test]
fn to_local_types_test_nested() {
    #[hdk_to_local_types]
    enum Nested1 {
        A,
        B,
    }

    #[hdk_to_local_types]
    enum Nested2 {
        X,
        Y,
        Z,
    }

    #[hdk_to_local_types]
    enum NoNesting {
        A(Nested1),
        #[allow(dead_code)]
        B {
            nested: Nested2,
        },
        C,
    }

    assert_eq!(local_type(NoNesting::A(Nested1::A)), LocalZomeTypeId(0u8));
    assert_eq!(local_type(NoNesting::A(Nested1::B)), LocalZomeTypeId(0u8));
    assert_eq!(local_type(&NoNesting::A(Nested1::A)), LocalZomeTypeId(0u8));
    assert_eq!(
        local_type(NoNesting::B { nested: Nested2::X }),
        LocalZomeTypeId(1u8)
    );
    assert_eq!(
        local_type(NoNesting::B { nested: Nested2::Y }),
        LocalZomeTypeId(1u8)
    );
    assert_eq!(
        local_type(NoNesting::B { nested: Nested2::Z }),
        LocalZomeTypeId(1u8)
    );
    assert_eq!(local_type(NoNesting::C), LocalZomeTypeId(2u8));

    #[hdk_to_local_types(nested)]
    enum Nesting {
        A(Nested1),
        #[allow(dead_code)]
        B {
            nested: Nested2,
        },
        C,
        D(Nested2),
    }

    assert_eq!(local_type(Nesting::A(Nested1::A)), LocalZomeTypeId(0u8));
    assert_eq!(local_type(Nesting::A(Nested1::B)), LocalZomeTypeId(1u8));
    assert_eq!(local_type(&Nesting::A(Nested1::A)), LocalZomeTypeId(0u8));
    assert_eq!(
        local_type(Nesting::B { nested: Nested2::X }),
        LocalZomeTypeId(2u8)
    );
    assert_eq!(
        local_type(Nesting::B { nested: Nested2::Y }),
        LocalZomeTypeId(3u8)
    );
    assert_eq!(
        local_type(Nesting::B { nested: Nested2::Z }),
        LocalZomeTypeId(4u8)
    );
    assert_eq!(local_type(Nesting::C), LocalZomeTypeId(5u8));
    assert_eq!(local_type(Nesting::D(Nested2::X)), LocalZomeTypeId(6u8));
    assert_eq!(local_type(Nesting::D(Nested2::Y)), LocalZomeTypeId(7u8));
    assert_eq!(local_type(Nesting::D(Nested2::Z)), LocalZomeTypeId(8u8));

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

    set_zome_types(&[], &[(0, 0..3)]);

    assert_eq!(
        zome_and_link_type(integrity_a::Unit::A),
        (ZomeId(0), LinkType(0))
    );
    assert_eq!(
        zome_and_link_type(&integrity_a::Unit::A),
        (ZomeId(0), LinkType(0))
    );
    assert_eq!(
        zome_and_link_type(integrity_a::Unit::B),
        (ZomeId(0), LinkType(1))
    );
    assert_eq!(
        zome_and_link_type(integrity_a::Unit::C),
        (ZomeId(0), LinkType(2))
    );

    set_zome_types(&[], &[(1, 0..3)]);

    assert_eq!(
        zome_and_link_type(integrity_b::Unit::A),
        (ZomeId(1), LinkType(0))
    );
    assert_eq!(
        zome_and_link_type(&integrity_b::Unit::A),
        (ZomeId(1), LinkType(0))
    );
    assert_eq!(
        zome_and_link_type(integrity_b::Unit::B),
        (ZomeId(1), LinkType(1))
    );
    assert_eq!(
        zome_and_link_type(integrity_b::Unit::C),
        (ZomeId(1), LinkType(2))
    );
}

mod entry_defs_to_entry_type_index_test {
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
    set_zome_types(&[(0, 0..3)], &[]);

    assert_eq!(
        zome_and_entry_type(integrity_a::EntryTypes::A(A {})),
        (ZomeId(0u8), EntryDefIndex(0))
    );
    assert_eq!(
        zome_and_entry_type(&integrity_a::EntryTypes::A(A {})),
        (ZomeId(0u8), EntryDefIndex(0))
    );
    assert_eq!(
        zome_and_entry_type(integrity_a::EntryTypes::B(B {})),
        (ZomeId(0u8), EntryDefIndex(1))
    );
    assert_eq!(
        zome_and_entry_type(integrity_a::EntryTypes::C(C {})),
        (ZomeId(0u8), EntryDefIndex(2))
    );

    assert!(matches!(
        integrity_a::EntryTypes::deserialize_from_type(0, 0, &Entry::try_from(A {}).unwrap()),
        Ok(Some(integrity_a::EntryTypes::A(A {})))
    ));
    assert!(matches!(
        integrity_a::EntryTypes::deserialize_from_type(0, 1, &Entry::try_from(A {}).unwrap()),
        Ok(Some(integrity_a::EntryTypes::B(B {})))
    ));
    assert!(matches!(
        integrity_a::EntryTypes::deserialize_from_type(0, 2, &Entry::try_from(A {}).unwrap()),
        Ok(Some(integrity_a::EntryTypes::C(C {})))
    ));

    assert!(matches!(
        integrity_a::EntryTypes::deserialize_from_type(0, 20, &Entry::try_from(A {}).unwrap()),
        Ok(None)
    ));
    assert!(matches!(
        integrity_a::EntryTypes::deserialize_from_type(1, 0, &Entry::try_from(A {}).unwrap()),
        Ok(None)
    ));

    // Set the integrity_b scope.
    set_zome_types(&[(1, 0..3)], &[]);

    assert_eq!(
        zome_and_entry_type(integrity_b::EntryTypes::A(A {})),
        (ZomeId(1u8), EntryDefIndex(0))
    );
    assert_eq!(
        zome_and_entry_type(&integrity_b::EntryTypes::A(A {})),
        (ZomeId(1u8), EntryDefIndex(0))
    );
    assert_eq!(
        zome_and_entry_type(integrity_b::EntryTypes::B(B {})),
        (ZomeId(1u8), EntryDefIndex(1))
    );
    assert_eq!(
        zome_and_entry_type(integrity_b::EntryTypes::C(C {})),
        (ZomeId(1u8), EntryDefIndex(2))
    );

    assert!(matches!(
        integrity_b::EntryTypes::deserialize_from_type(1, 0, &Entry::try_from(A {}).unwrap()),
        Ok(Some(integrity_b::EntryTypes::A(A {})))
    ));
    assert!(matches!(
        integrity_b::EntryTypes::deserialize_from_type(1, 1, &Entry::try_from(A {}).unwrap()),
        Ok(Some(integrity_b::EntryTypes::B(B {})))
    ));
    assert!(matches!(
        integrity_b::EntryTypes::deserialize_from_type(1, 2, &Entry::try_from(A {}).unwrap()),
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
    set_zome_types(&[], &[(0, 0..3)]);
    assert_eq!(LinkTypes::try_from(LocalZomeTypeId(0)), Ok(LinkTypes::A));
    assert_eq!(LinkTypes::try_from(&LocalZomeTypeId(0)), Ok(LinkTypes::A));
    assert!(matches!(LinkTypes::try_from(LocalZomeTypeId(50)), Err(_)));
    assert_eq!(LinkTypes::try_from(LocalZomeTypeId(1)), Ok(LinkTypes::B));
    assert_eq!(LinkTypes::try_from(LocalZomeTypeId(2)), Ok(LinkTypes::C));

    assert_eq!(
        LinkTypes::try_from((ZomeId(0), LinkType(0))),
        Ok(LinkTypes::A)
    );
    assert!(matches!(
        LinkTypes::try_from((ZomeId(0), LinkType(3))),
        Err(_)
    ));
    assert!(matches!(
        LinkTypes::try_from((ZomeId(1), LinkType(0))),
        Err(_)
    ));
    assert_eq!(
        LinkTypes::try_from((ZomeId(0), LinkType(1))),
        Ok(LinkTypes::B)
    );
    assert_eq!(
        LinkTypes::try_from((ZomeId(0), LinkType(2))),
        Ok(LinkTypes::C)
    );
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

fn set_zome_types(entries: &[(u8, Range<u8>)], links: &[(u8, Range<u8>)]) {
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
                .flat_map(|(z, types)| types.clone().map(|t| (LocalZomeTypeId(t), ZomeId(*z))))
                .collect(),
        ),
        links: ScopedZomeTypes(
            links
                .into_iter()
                .flat_map(|(z, types)| types.clone().map(|t| (LocalZomeTypeId(t), ZomeId(*z))))
                .collect(),
        ),
    }));
}
