use std::ops::Range;

use holochain_deterministic_integrity::prelude::*;

fn local_type(t: impl Into<LocalZomeTypeId>) -> LocalZomeTypeId {
    t.into()
}

fn global_type(t: impl TryInto<GlobalZomeTypeId, Error = WasmError>) -> GlobalZomeTypeId {
    match t.try_into() {
        Ok(t) => t,
        Err(e) => panic!("Failed to convert to global zome type id: {:?}", e),
    }
}

fn entry_index(t: impl TryInto<EntryDefIndex, Error = WasmError>) -> EntryDefIndex {
    match t.try_into() {
        Ok(t) => t,
        Err(e) => panic!("Failed to convert to entry def index: {:?}", e),
    }
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
fn to_global_types_test_unit() {
    mod integrity_a {
        use super::*;
        #[hdk_to_local_types]
        #[hdk_to_global_entry_types]
        #[derive(Debug)]
        pub enum Unit {
            A,
            B,
            C,
        }
    }

    mod integrity_b {
        use super::*;
        #[hdk_to_local_types]
        #[hdk_to_global_entry_types]
        #[derive(Debug)]
        pub enum Unit {
            A,
            B,
            C,
        }
    }

    set_zome_types(vec![0..3], vec![]);

    assert_eq!(global_type(integrity_a::Unit::A), GlobalZomeTypeId(0u8));
    assert_eq!(global_type(&integrity_a::Unit::A), GlobalZomeTypeId(0u8));
    assert_eq!(global_type(integrity_a::Unit::B), GlobalZomeTypeId(1u8));
    assert_eq!(global_type(integrity_a::Unit::C), GlobalZomeTypeId(2u8));

    set_zome_types(vec![3..6], vec![]);

    assert_eq!(global_type(integrity_b::Unit::A), GlobalZomeTypeId(3u8));
    assert_eq!(global_type(&integrity_b::Unit::A), GlobalZomeTypeId(3u8));
    assert_eq!(global_type(integrity_b::Unit::B), GlobalZomeTypeId(4u8));
    assert_eq!(global_type(integrity_b::Unit::C), GlobalZomeTypeId(5u8));
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
    set_zome_types(vec![0..3], vec![]);

    assert_eq!(
        global_type(integrity_a::EntryTypes::A(A {})),
        GlobalZomeTypeId(0u8)
    );
    assert_eq!(
        global_type(&integrity_a::EntryTypes::A(A {})),
        GlobalZomeTypeId(0u8)
    );
    assert_eq!(
        global_type(integrity_a::EntryTypes::B(B {})),
        GlobalZomeTypeId(1u8)
    );
    assert_eq!(
        global_type(integrity_a::EntryTypes::C(C {})),
        GlobalZomeTypeId(2u8)
    );

    assert_eq!(
        entry_index(integrity_a::EntryTypes::A(A {})),
        EntryDefIndex(0u8)
    );
    assert_eq!(
        entry_index(&integrity_a::EntryTypes::A(A {})),
        EntryDefIndex(0u8)
    );
    assert_eq!(
        entry_index(integrity_a::EntryTypes::B(B {})),
        EntryDefIndex(1u8)
    );
    assert_eq!(
        entry_index(integrity_a::EntryTypes::C(C {})),
        EntryDefIndex(2u8)
    );

    assert!(matches!(
        integrity_a::EntryTypes::try_from_global_type(0u8, &Entry::try_from(A {}).unwrap()),
        Ok(Some(integrity_a::EntryTypes::A(A {})))
    ));
    assert!(matches!(
        integrity_a::EntryTypes::try_from_global_type(1u8, &Entry::try_from(B {}).unwrap()),
        Ok(Some(integrity_a::EntryTypes::B(B {})))
    ));
    assert!(matches!(
        integrity_a::EntryTypes::try_from_global_type(2u8, &Entry::try_from(C {}).unwrap()),
        Ok(Some(integrity_a::EntryTypes::C(C {})))
    ));

    // Set the integrity_b scope.
    set_zome_types(vec![3..6], vec![]);

    assert_eq!(
        global_type(integrity_b::EntryTypes::A(A {})),
        GlobalZomeTypeId(3u8)
    );
    assert_eq!(
        global_type(&integrity_b::EntryTypes::A(A {})),
        GlobalZomeTypeId(3u8)
    );
    assert_eq!(
        global_type(integrity_b::EntryTypes::B(B {})),
        GlobalZomeTypeId(4u8)
    );
    assert_eq!(
        global_type(integrity_b::EntryTypes::C(C {})),
        GlobalZomeTypeId(5u8)
    );

    assert_eq!(
        entry_index(integrity_b::EntryTypes::A(A {})),
        EntryDefIndex(3u8)
    );
    assert_eq!(
        entry_index(&integrity_b::EntryTypes::A(A {})),
        EntryDefIndex(3u8)
    );
    assert_eq!(
        entry_index(integrity_b::EntryTypes::B(B {})),
        EntryDefIndex(4u8)
    );
    assert_eq!(
        entry_index(integrity_b::EntryTypes::C(C {})),
        EntryDefIndex(5u8)
    );

    assert!(matches!(
        integrity_b::EntryTypes::try_from_global_type(3u8, &Entry::try_from(A {}).unwrap()),
        Ok(Some(integrity_b::EntryTypes::A(A {})))
    ));
    assert!(matches!(
        integrity_b::EntryTypes::try_from_global_type(4u8, &Entry::try_from(B {}).unwrap()),
        Ok(Some(integrity_b::EntryTypes::B(B {})))
    ));
    assert!(matches!(
        integrity_b::EntryTypes::try_from_global_type(5u8, &Entry::try_from(C {}).unwrap()),
        Ok(Some(integrity_b::EntryTypes::C(C {})))
    ));
}

#[test]
fn link_types_from_header() {
    #[hdk_link_types(skip_no_mangle = true)]
    pub enum LinkTypes {
        A,
        B,
        C,
    }
    set_zome_types(vec![], vec![50..53]);
    assert_eq!(LinkTypes::try_from(LocalZomeTypeId(0)), Ok(LinkTypes::A));
    assert_eq!(LinkTypes::try_from(&LocalZomeTypeId(0)), Ok(LinkTypes::A));
    assert!(matches!(LinkTypes::try_from(LocalZomeTypeId(50)), Err(_)));
    assert_eq!(LinkTypes::try_from(LocalZomeTypeId(1)), Ok(LinkTypes::B));
    assert_eq!(LinkTypes::try_from(LocalZomeTypeId(2)), Ok(LinkTypes::C));

    assert_eq!(LinkTypes::try_from(LinkType(50)), Ok(LinkTypes::A));
    assert_eq!(LinkTypes::try_from(&LinkType(50)), Ok(LinkTypes::A));
    assert!(matches!(LinkTypes::try_from(LinkType(0)), Err(_)));
    assert_eq!(LinkTypes::try_from(LinkType(51)), Ok(LinkTypes::B));
    assert_eq!(LinkTypes::try_from(LinkType(52)), Ok(LinkTypes::C));
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

fn set_zome_types(entries: Vec<Range<u8>>, links: Vec<Range<u8>>) {
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

        fn must_get_header(
            &self,
            must_get_header_input: MustGetHeaderInput,
        ) -> ExternResult<SignedHeaderHashed> {
            todo!()
        }

        fn must_get_valid_element(
            &self,
            must_get_valid_element_input: MustGetValidElementInput,
        ) -> ExternResult<Element> {
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
                .map(|r| GlobalZomeTypeId(r.start)..GlobalZomeTypeId(r.end))
                .collect(),
        ),
        links: ScopedZomeTypes(
            links
                .into_iter()
                .map(|r| GlobalZomeTypeId(r.start)..GlobalZomeTypeId(r.end))
                .collect(),
        ),
    }));
}
