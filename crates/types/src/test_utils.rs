//! Some common testing helpers.

use crate::{
    cell::CellId,
    dna::{wasm::DnaWasm, zome::Zome, Dna},
    prelude::*,
    shims::{CapToken, CapabilityRequest},
    signature::{Provenance, Signature},
};
use holo_hash::AgentHash;
use std::path::PathBuf;
use sx_zome_types::ZomeExternHostInput;

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
    Zome {}
}

/// A fixture example dna for unit testing.
pub fn fake_dna(uuid: &str) -> Dna {
    Dna {}
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
    (name.to_string().try_into().unwrap(), fake_agent_hash(name)).into()
}

/// A fixture example AgentHash for unit testing.
pub fn fake_agent_hash(_name: &str) -> AgentHash {
    todo!("Implement fake Agent fixture and get its hash")
}

/// A fixture example AgentHash for unit testing.
pub fn fake_header_hash(_name: &str) -> HeaderHash {
    todo!("Implement fake Header fixture and get its hash")
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

/// A fixture example Signature for unit testing.
pub fn fake_signature() -> Signature {
    Signature::from("fake")
}

/// A fixture example Provenance for unit testing.
pub fn fake_provenance() -> Provenance {
    Provenance::new(
        AgentHash::try_from("fake").expect("TODO, will fail"),
        fake_signature(),
    )
}
