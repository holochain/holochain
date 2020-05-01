#![deny(missing_docs)]
//! DnaFile Utilities

use holochain_serialized_bytes::prelude::*;
use holochain_types::dna::{DnaDef, DnaFile};
use std::{collections::BTreeMap, convert::TryFrom};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// DnaUtilError type.
#[derive(Debug, thiserror::Error)]
pub enum DnaUtilError {
    /// std::io::Error
    #[error("std::io::Error: {0}")]
    StdIoError(#[from] std::io::Error),

    /// DnaError
    #[error("DnaError: {0}")]
    DnaError(#[from] holochain_types::dna::DnaError),

    /// SerializedBytesError
    #[error("SerializedBytesError: {0}")]
    SerializedBytesError(#[from] SerializedBytesError),

    /// InvalidInput
    #[error("InvalidInput: {0}")]
    InvalidInput(String),

    /// serde_json::Error
    #[error("serde_json::Error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
}

/// DnaUtil Result type.
pub type DnaUtilResult<T> = Result<T, DnaUtilError>;

/// Extract a DnaFile into a Dna Working Directory
pub async fn extract(dna_file_path: impl AsRef<std::path::Path>) -> DnaUtilResult<()> {
    let dna_file_path = dna_file_path.as_ref().canonicalize()?;
    if !dna_file_path.to_string_lossy().ends_with(".dna.gz") {
        return Err(DnaUtilError::InvalidInput(format!(
            "bad extract path, dna files must end with '.dna.gz': {}",
            dna_file_path.display()
        )));
    }
    let filename = dna_file_path
        .file_name()
        .ok_or_else(|| {
            DnaUtilError::InvalidInput(format!(
                "could not extract filename from: {}",
                dna_file_path.display()
            ))
        })?
        .to_string_lossy();
    let filename_base = &filename[..filename.len() - 7];
    let dirname = format!("{}.dna_work_dir", filename_base);
    let mut dir = dna_file_path.clone();
    dir.set_file_name(dirname);
    tokio::fs::create_dir_all(&dir).await?;

    let mut file = tokio::fs::File::open(dna_file_path).await?;
    let mut data = Vec::new();
    file.read_to_end(&mut data).await?;
    let dna_file = DnaFile::from_file_content(&data).await?;

    for (zome_name, zome) in &dna_file.dna().zomes {
        let wasm_hash = &zome.wasm_hash;
        let wasm = dna_file.code().get(wasm_hash).expect("dna_file corrupted");
        let mut wasm_filename = dir.clone();
        wasm_filename.push(format!("{}.wasm", zome_name));
        let mut wasm_file = tokio::fs::File::create(wasm_filename).await?;
        wasm_file.write_all(&wasm.code()).await?;
    }

    let dna_json: DnaDefJson = dna_file.dna().clone().try_into()?;
    let dna_json = serde_json::to_string_pretty(&dna_json)?;

    let mut json_filename = dir.clone();
    json_filename.push("dna.json");
    let mut json_file = tokio::fs::File::create(json_filename).await?;
    json_file.write_all(dna_json.as_bytes()).await?;

    Ok(())
}

/// See `holochain_types::dna::zome::Zome`.
/// This is a helper to convert to json.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ZomeJson {
    pub wasm_path: String,
}

/// Special Json Value Decode Helper
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
struct JsonValueDecodeHelper(pub serde_json::Value);

/// See `holochain_types::dna::DnaDef`.
/// This is a helper to convert to json.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct DnaDefJson {
    pub name: String,
    pub uuid: String,
    pub properties: serde_json::Value,
    pub zomes: BTreeMap<String, ZomeJson>,
}

impl TryFrom<DnaDef> for DnaDefJson {
    type Error = SerializedBytesError;

    fn try_from(dna: DnaDef) -> Result<Self, SerializedBytesError> {
        let properties: JsonValueDecodeHelper = dna.properties.try_into()?;
        let mut zomes = BTreeMap::new();
        for zome_name in dna.zomes.keys() {
            let zome_file = format!("./{}.wasm", zome_name);
            zomes.insert(
                zome_name.clone(),
                ZomeJson {
                    wasm_path: zome_file,
                },
            );
        }
        Ok(Self {
            name: dna.name,
            uuid: dna.uuid,
            properties: properties.0,
            zomes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(threaded_scheduler)]
    async fn test_extract() {
        let tmp_dir = tempdir::TempDir::new("dna_util_test").unwrap();
        println!("HANAH: {:?}", tmp_dir.path());

        let dna_file = holochain_types::test_utils::fake_dna_zomes(
            "bla",
            vec![
                ("test-zome-1".into(), vec![1, 2, 3, 4].into()),
                ("test-zome-2".into(), vec![5, 6, 7, 8].into()),
            ],
        );

        let dna_filename = tmp_dir.path().join("test-dna.dna.gz");

        {
            let mut file = tokio::fs::File::create(&dna_filename).await.unwrap();
            file.write_all(&dna_file.as_file_content().await.unwrap())
                .await
                .unwrap();
        }

        extract(dna_filename).await.unwrap();
    }
}
