//! Some common testing helpers.

use crate::{
    cell::CellId,
    dna::{wasm::DnaWasm, zome::Zome, DnaDef, DnaFile},
    prelude::*,
    shims::CapToken,
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
        wasm_hash: holo_hash_core::WasmHash::new(vec![0; 36]),
    }
}

/// A fixture example dna for unit testing.
pub fn fake_dna(uuid: &str) -> DnaFile {
    fake_dna_zomes(uuid, vec![("test".into(), vec![].into())])
}

/// A fixture example dna for unit testing.
pub fn fake_dna_zomes(uuid: &str, zomes: Vec<(String, DnaWasm)>) -> DnaFile {
    let mut dna = DnaDef {
        name: "test".to_string(),
        properties: ().try_into().unwrap(),
        uuid: uuid.to_string(),
        zomes: BTreeMap::new(),
    };
    let mut wasm_code = Vec::new();
    for (zome_name, wasm) in zomes {
        let wasm_hash = holo_hash::WasmHash::with_data_sync(&wasm.code());
        let wasm_hash: holo_hash_core::WasmHash = wasm_hash.into();
        dna.zomes.insert(zome_name, Zome { wasm_hash });
        wasm_code.push(wasm);
    }
    tokio_safe_block_on::tokio_safe_block_on(
        DnaFile::new(dna, wasm_code),
        std::time::Duration::from_secs(1),
    )
    .unwrap()
    .unwrap()
}

/// Save a Dna to a file and return the path and tempdir that contains it
pub fn write_fake_dna_file(dna: DnaFile) -> anyhow::Result<(PathBuf, tempdir::TempDir)> {
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
    holo_hash::AgentPubKey::try_from("uhCAkw-zrttiYpdfAYX4fR6W8DPUdheZJ-1QsRA4cTImmzTYUcOr4")
        .unwrap()
}

/// Another fixture example AgentPubKey for unit testing.
pub fn fake_agent_pubkey_2() -> AgentPubKey {
    holo_hash::AgentPubKey::try_from("uhCAkomHzekU0-x7p62WmrusdxD2w9wcjdajC88688JGSTEo6cbEK")
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

/// A fixture example ZomeInvocationPayload for unit testing.
pub fn fake_zome_invocation_payload() -> ZomeExternHostInput {
    ZomeExternHostInput::try_from(SerializedBytes::try_from(()).unwrap()).unwrap()
}
