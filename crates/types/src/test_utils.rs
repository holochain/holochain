//! Some common testing helpers.

use crate::{
    cell::CellId,
    dna::{wasm::DnaWasm, zome::Zome, JsonProperties},
    dna::{DnaDef, DnaFile},
    prelude::*,
};
use holochain_zome_types::capability::CapSecret;
use holochain_zome_types::capability::CAP_SECRET_BYTES;
use holochain_zome_types::zome::ZomeName;
use std::path::PathBuf;

pub use holochain_zome_types::test_utils::*;

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
        wasm_hash: holo_hash::WasmHash::from_raw_32(vec![0; 32]),
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

/// Keeping with convention if Alice is pubkey 1
/// and bob is pubkey 2 the this helps make test
/// logging easier to read.
pub fn which_agent(key: &AgentPubKey) -> String {
    let key = key.to_string();
    let alice = fake_agent_pubkey_1().to_string();
    let bob = fake_agent_pubkey_2().to_string();
    if key == alice {
        return "alice".to_string();
    }
    if key == bob {
        return "bob".to_string();
    }
    key
}

/// A fixture CapSecret for unit testing.
pub fn fake_cap_secret() -> CapSecret {
    [0; CAP_SECRET_BYTES].into()
}
