//! Some common testing helpers.

use crate::{
    dna::{
        bridges::Bridge,
        entry_types::EntryTypeDef,
        fn_declarations::{FnDeclaration, TraitFns},
        wasm::DnaWasm,
        zome::{Config, Zome, ZomeFnDeclarations},
        Dna,
    },
    prelude::*,
};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, SerializedBytes)]
struct TestProperties {
    test: String,
}

/// simple EntryTypeDef fixture
pub fn test_entry_type() -> EntryTypeDef {
    EntryTypeDef {
        ..Default::default()
    }
}

/// simple TraitFns fixture
pub fn test_traits() -> TraitFns {
    TraitFns {
        functions: vec![String::from("test")],
    }
}

/// simple ZomeFnDeclarations fixture
pub fn test_fn_declarations() -> ZomeFnDeclarations {
    vec![FnDeclaration {
        name: "test".into(),
        inputs: vec![],
        outputs: vec![],
    }]
}

/// simple DnaWasm fixture
pub fn test_code() -> DnaWasm {
    DnaWasm::from(vec![0_u8])
}

/// simple Bridges fixture
pub fn test_bridges() -> Vec<Bridge> {
    vec![]
}

/// simple Zome fixture
pub fn test_zome() -> Zome {
    Zome {
        description: "test".into(),
        config: Config::default(),
        entry_types: {
            let mut v = BTreeMap::new();
            v.insert("test".into(), test_entry_type());
            v
        },
        traits: {
            let mut v = BTreeMap::new();
            v.insert("hc_public".into(), test_traits());
            v
        },
        fn_declarations: test_fn_declarations(),
        code: test_code(),
        bridges: test_bridges(),
    }
}

/// A fixture example dna for unit testing.
pub fn test_dna(uuid: &str) -> Dna {
    Dna {
        name: "test".into(),
        description: "test".into(),
        version: "test".into(),
        uuid: uuid.into(),
        properties: TestProperties {
            test: "test".into(),
        }
        .try_into()
        .unwrap(),
        zomes: {
            let mut v = BTreeMap::new();
            v.insert("test".into(), test_zome());
            v
        },
        ..Default::default()
    }
}
