use super::dna::wasm::DnaWasm;
use super::dna::zome::WasmZome;
use super::dna::DnaDef;
use super::dna::DnaFile;
use holochain_types::app::JsonProperties;
use holochain_zome_types::zome::ZomeName;
use std::convert::TryInto;
use std::path::PathBuf;

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
            let wasm = crate::nucleus::dna::wasm::DnaWasmHashed::from_content(wasm).await;
            let (wasm, wasm_hash) = wasm.into_inner();
            dna.zomes.push((zome_name, WasmZome { wasm_hash }.into()));
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
