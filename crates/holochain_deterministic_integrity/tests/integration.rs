use holochain_deterministic_integrity::prelude::*;

fn local_type(t: impl Into<LocalZomeTypeId>) -> LocalZomeTypeId {
    t.into()
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

    // #[hdk_to_local_types(nested)]
    enum Nesting {
        A(Nested1),
        #[allow(dead_code)]
        B {
            nested: Nested2,
        },
        C,
        D(Nested2),
    }

    impl Nesting {
        fn len() -> u8 {
            Nested1::len() + Nested2::len() + 1
        }
    }

    impl From<Nesting> for LocalZomeTypeId {
        fn from(n: Nesting) -> Self {
            match n {
                Nesting::A(inner) => Self(0 + Self::from(inner).0),
                Nesting::B { nested } => Self(0 + Nested1::len() + Self::from(nested).0),
                Nesting::C => Self(0 + Nested1::len() + Nested2::len()),
                Nesting::D(inner) => {
                    Self(0 + Nested1::len() + Nested2::len() + 1 + Self::from(inner).0)
                }
            }
        }
    }
    assert_eq!(local_type(Nesting::A(Nested1::A)), LocalZomeTypeId(0u8));
    assert_eq!(local_type(Nesting::A(Nested1::B)), LocalZomeTypeId(1u8));
    // assert_eq!(local_type(&Nesting::A(Nested1::A)), LocalZomeTypeId(0u8));
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
}
