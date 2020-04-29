//! Some common testing helpers.

use crate::{
    cell::CellId,
    dna::{wasm::DnaWasm, zome::Zome, Dna},
    prelude::*,
    shims::{CapToken, CapabilityRequest},
    signature::Provenance,
};
use holo_hash::AgentPubKey;
use holochain_zome_types::ZomeExternHostInput;
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Serialize, Deserialize, SerializedBytes)]
struct FakeProperties {
    test: String,
}

/// simple DnaWasm fixture
pub fn fake_dna_wasm() -> DnaWasm {
    DnaWasm::from(vec![0_u8])
}

/// simple Zome fixture
pub fn fake_zome() -> Zome {
    Zome {
        code: fake_dna_wasm(),
    }
}

/// A fixture example dna for unit testing.
pub fn fake_dna(uuid: &str) -> Dna {
    Dna {
        name: "test".to_string(),
        properties: ().try_into().unwrap(),
        uuid: uuid.to_string(),
        zomes: {
            let mut v = BTreeMap::new();
            v.insert("test".into(), fake_zome());
            v
        },
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
    (fake_dna_hash(name), fake_agent_pubkey_1()).into()
}

/// A fixture example DnaHash for unit testing.
pub fn fake_dna_hash(name: &str) -> DnaHash {
    DnaHash::with_data_sync(name.as_bytes())
}

/// A fixture example AgentPubKey for unit testing.
pub fn fake_agent_pubkey_1() -> AgentPubKey {
    holo_hash::AgentPubKey::try_from(
        "uhCAkw-zrttiYpdfAYX4fR6W8DPUdheZJ-1QsRA4cTImmzTYUcOr4",
    )
    .unwrap()
}

/// Another fixture example AgentPubKey for unit testing.
pub fn fake_agent_pubkey_2() -> AgentPubKey {
    holo_hash::AgentPubKey::try_from(
        "uhCAkomHzekU0-x7p62WmrusdxD2w9wcjdajC88688JGSTEo6cbEK",
    )
    .unwrap()
}

/// A fixture example HeaderHash for unit testing.
pub fn fake_header_hash(name: &str) -> HeaderHash {
    HeaderHash::with_data_sync(name.as_bytes())
}

/// A fixture example CapabilityRequest for unit testing.
pub fn fake_cap_token() -> CapToken {
    // TODO: real fake CapToken
    CapToken
}

/// A fixture example CapabilityRequest for unit testing.
pub fn fake_capability_request() -> CapabilityRequest {
    CapabilityRequest::new(CapToken, fake_provenance())
}

/// A fixture example ZomeInvocationPayload for unit testing.
pub fn fake_zome_invocation_payload() -> ZomeExternHostInput {
    ZomeExternHostInput::try_from(SerializedBytes::try_from(()).unwrap()).unwrap()
}

/// A fixture example Provenance for unit testing.
pub fn fake_provenance() -> Provenance {
    let fake_agent = AgentPubKey::with_pre_hashed_sync(vec![0; 32]);
    fake_provenance_for_agent(&fake_agent)
}

/// A fixture example Provenance for unit testing.
pub fn fake_provenance_for_agent(agent_pubkey: &AgentPubKey) -> Provenance {
    let agent_pubkey = agent_pubkey.clone();

    let fake_signature = Signature(vec![0; 32]);

    Provenance::new(agent_pubkey, fake_signature)
}
