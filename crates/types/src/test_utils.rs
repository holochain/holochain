//! Some common testing helpers.

use crate::{
    agent::AgentId,
    cell::CellId,
    dna::{
        bridges::Bridge,
        capabilities::CapabilityRequest,
        entry_types::EntryTypeDef,
        fn_declarations::{FnDeclaration, TraitFns},
        wasm::DnaWasm,
        zome::{Config, Zome, ZomeFnDeclarations},
        Dna,
    },
    prelude::*,
    signature::{Provenance, Signature},
};
use std::{collections::BTreeMap, path::PathBuf};
use sx_zome_types::ZomeExternHostInput;

#[derive(Serialize, Deserialize, SerializedBytes)]
struct FakeProperties {
    test: String,
}

/// simple EntryTypeDef fixture
pub fn fake_entry_type() -> EntryTypeDef {
    EntryTypeDef {
        ..Default::default()
    }
}

/// simple TraitFns fixture
pub fn fake_traits() -> TraitFns {
    TraitFns {
        functions: vec![String::from("test")],
    }
}

/// simple ZomeFnDeclarations fixture
pub fn fake_fn_declarations() -> ZomeFnDeclarations {
    vec![FnDeclaration {
        name: "test".into(),
        inputs: vec![],
        outputs: vec![],
    }]
}

/// simple DnaWasm fixture
pub fn fake_dna_wasm() -> DnaWasm {
    DnaWasm::from(vec![0_u8])
}

/// simple Bridges fixture
pub fn fake_bridges() -> Vec<Bridge> {
    vec![]
}

/// simple Zome fixture
pub fn fake_zome() -> Zome {
    Zome {
        description: "test".into(),
        config: Config::default(),
        entry_types: {
            let mut v = BTreeMap::new();
            v.insert("test".into(), fake_entry_type());
            v
        },
        traits: {
            let mut v = BTreeMap::new();
            v.insert("hc_public".into(), fake_traits());
            v
        },
        fn_declarations: fake_fn_declarations(),
        code: fake_dna_wasm(),
        bridges: fake_bridges(),
    }
}

/// A fixture example dna for unit testing.
pub fn fake_dna(uuid: &str) -> Dna {
    Dna {
        name: "test".into(),
        description: "test".into(),
        version: "test".into(),
        uuid: uuid.into(),
        properties: FakeProperties {
            test: "test".into(),
        }
        .try_into()
        .unwrap(),
        zomes: {
            let mut v = BTreeMap::new();
            v.insert("test".into(), fake_zome());
            v
        },
        dna_spec_version: Default::default(),
    }
}

/// Save a Dna to a file and return the path and tempdir that contains it
pub fn fake_dna_file(dna: Dna) -> anyhow::Result<(PathBuf, tempdir::TempDir)> {
    let tmp_dir = tempdir::TempDir::new("fake_dna")?;
    let mut path: PathBuf = tmp_dir.path().into();
    path.push("dna");
    std::fs::write(path.clone(), SerializedBytes::try_from(dna)?.bytes())?;
    Ok((path, tmp_dir))
}

/// A fixture example CellId for unit testing.
pub fn fake_cell_id(name: &str) -> CellId {
    (name.to_string().into(), fake_agent_id(name)).into()
}

/// A fixture example AgentId for unit testing.
pub fn fake_agent_id(name: &str) -> AgentId {
    AgentId::generate_fake(name)
}

/// A fixture example CapabilityRequest for unit testing.
pub fn fake_capability_request() -> CapabilityRequest {
    CapabilityRequest {
        cap_token: Address::from("fake"),
        provenance: fake_provenance(),
    }
}

/// A fixture example ZomeInvocationPayload for unit testing.
pub fn fake_zome_invocation_payload() -> ZomeExternHostInput {
    ZomeExternHostInput::try_from(SerializedBytes::try_from(()).unwrap()).unwrap()
}

/// A fixture example Signature for unit testing.
pub fn fake_signature() -> Signature {
    Signature::from("fake")
}

/// A fixture example Provenance for unit testing.
pub fn fake_provenance() -> Provenance {
    Provenance::new("fake".into(), fake_signature())
}
