use sx_fixture::*;
use super::Dna;
use std::collections::BTreeMap;
use crate::prelude::*;
use crate::dna::zome::Config;
use crate::dna::zome::Zome;
use crate::dna::entry_types::EntryTypeDef;
use crate::dna::fn_declarations::TraitFns;
use crate::dna::fn_declarations::FnDeclaration;
use crate::dna::wasm::DnaWasm;

impl Fixture for DnaWasm {
    type Input = ();
    fn fixture(_: FixtureType<Self::Input>) -> DnaWasm {
        DnaWasm::from(vec![0_u8])
    }
}

impl Fixture for FnDeclaration {
    type Input = ();
    fn fixture(_: FixtureType<Self::Input>) -> FnDeclaration {
        FnDeclaration {
            name: "test".into(),
            inputs: vec![],
            outputs: vec![],
        }
    }
}

impl Fixture for TraitFns {
    type Input = ();
    fn fixture(_: FixtureType<Self::Input>) -> TraitFns {
        TraitFns {
            functions: vec![String::from("test")],
        }
    }
}

impl Fixture for EntryTypeDef {
    type Input = ();
    fn fixture(_: FixtureType<Self::Input>) -> EntryTypeDef {
        EntryTypeDef::default()
    }
}

impl Fixture for Zome {
    type Input = ();
    fn fixture(_: FixtureType<Self::Input>) -> Zome {
        Zome {
            description: "test".into(),
            config: Config::default(),
            entry_types: {
                let mut v = BTreeMap::new();
                v.insert("test".into(), EntryTypeDef::fixture(FixtureType::A));
                v
            },
            traits: {
                let mut v = BTreeMap::new();
                v.insert("hc_public".into(), TraitFns::fixture(FixtureType::A));
                v
            },
            fn_declarations: vec![FnDeclaration::fixture(FixtureType::A)],
            code: DnaWasm::fixture(FixtureType::A),
            bridges: vec![],
        }
    }
}

pub struct DnaFixtureInput {
    pub uuid: String,
}

#[derive(Serialize, Deserialize, SerializedBytes)]
struct FakeProperties {
    test: String,
}

impl Fixture for Dna {
    type Input = DnaFixtureInput;
    fn fixture(fixture_type: FixtureType<Self::Input>) -> Dna {
        match fixture_type {
            FixtureType::A => Dna::fixture(FixtureType::FromInput(DnaFixtureInput { uuid: "a".into() })),
            FixtureType::FromInput(input) => {
                Dna {
                    name: "test".into(),
                    description: "test".into(),
                    version: "test".into(),
                    uuid: input.uuid.into(),
                    properties: FakeProperties {
                        test: "test".into(),
                    }
                    .try_into()
                    .unwrap(),
                    zomes: {
                        let mut v = BTreeMap::new();
                        v.insert("test".into(), Zome::fixture(FixtureType::A));
                        v
                    },
                    dna_spec_version: Default::default(),
                }
            },
            _ => unimplemented!(),
        }
    }
}
