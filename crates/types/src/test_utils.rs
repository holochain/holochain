//! Some common testing helpers.

use crate::{
    cell::CellId,
    dna::{wasm::DnaWasm, zome::Zome, JsonProperties},
    dna::{DnaDef, DnaFile},
    prelude::*,
};

use holo_hash::{hash_type, PrimitiveHashType};
use holochain_zome_types::capability::CapSecret;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::HostInput;
use std::path::PathBuf;

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
        wasm_hash: holo_hash::WasmHash::from_raw_bytes(vec![0; 36]),
    }
}

/// A fixture example dna for unit testing.
pub fn fake_dna_file(uuid: &str) -> DnaFile {
    fake_dna_zomes(uuid, vec![("test".into(), vec![].into())])
}

/// A fixture example dna for unit testing.
pub fn fake_dna_zomes(uuid: &str, zomes: Vec<(ZomeName, DnaWasm)>) -> DnaFile {
    let mut dna = DnaDef {
        name: "test".to_string(),
        properties: JsonProperties::new(serde_json::json!({"p": "hi"}))
            .try_into()
            .unwrap(),
        uuid: uuid.to_string(),
        zomes: Vec::new(),
    };
    tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        let mut wasm_code = Vec::new();
        for (zome_name, wasm) in zomes {
            let wasm = crate::dna::wasm::DnaWasmHashed::from_content(wasm).await;
            let (wasm, wasm_hash) = wasm.into_inner();
            dna.zomes.push((zome_name, Zome { wasm_hash }));
            wasm_code.push(wasm);
        }
        DnaFile::new(dna, wasm_code).await
    })
    .unwrap()
}

/// Save a Dna to a file and return the path and tempdir that contains it
pub async fn write_fake_dna_file(dna: DnaFile) -> anyhow::Result<(PathBuf, tempdir::TempDir)> {
    let tmp_dir = tempdir::TempDir::new("fake_dna")?;
    let mut path: PathBuf = tmp_dir.path().into();
    path.push("test-dna.dna.gz");
    tokio::fs::write(path.clone(), dna.to_file_content().await?).await?;
    Ok((path, tmp_dir))
}

/// A fixture example CellId for unit testing.
pub fn fake_cell_id(name: u8) -> CellId {
    (fake_dna_hash(name), fake_agent_pubkey_1()).into()
}

fn fake_holo_hash<T: holo_hash::HashType>(name: u8, hash_type: T) -> HoloHash<T> {
    HoloHash::from_raw_bytes_and_type([name; 36].to_vec(), hash_type)
}

/// A fixture DnaHash for unit testing.
pub fn fake_dna_hash(name: u8) -> DnaHash {
    fake_holo_hash(name, hash_type::Dna::new())
}

/// A fixture HeaderHash for unit testing.
pub fn fake_header_hash(name: u8) -> HeaderHash {
    fake_holo_hash(name, hash_type::Header::new())
}

/// A fixture DhtOpHash for unit testing.
pub fn fake_dht_op_hash(name: u8) -> DhtOpHash {
    fake_holo_hash(name, hash_type::DhtOp::new())
}

/// A fixture EntryContentHash for unit testing.
pub fn fake_entry_content_hash(name: u8) -> EntryContentHash {
    fake_holo_hash(name, hash_type::Content::new())
}

/// A fixture AgentPubKey for unit testing.
pub fn fake_agent_pub_key(name: u8) -> AgentPubKey {
    fake_holo_hash(name, hash_type::Agent::new())
}

/// A fixture AgentPubKey for unit testing.
pub fn fake_agent_pubkey_1() -> AgentPubKey {
    holo_hash_ext::AgentPubKey::try_from("uhCAkw-zrttiYpdfAYX4fR6W8DPUdheZJ-1QsRA4cTImmzTYUcOr4")
        .unwrap()
}

/// Another fixture AgentPubKey for unit testing.
pub fn fake_agent_pubkey_2() -> AgentPubKey {
    holo_hash_ext::AgentPubKey::try_from("uhCAkomHzekU0-x7p62WmrusdxD2w9wcjdajC88688JGSTEo6cbEK")
        .unwrap()
}

/// A fixture CapSecret for unit testing.
pub fn fake_cap_secret() -> CapSecret {
    CapSecret::random()
}

/// A fixture ZomeCallInvocationPayload for unit testing.
pub fn fake_zome_invocation_payload() -> HostInput {
    HostInput::try_from(SerializedBytes::try_from(()).unwrap()).unwrap()
}
