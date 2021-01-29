#![forbid(missing_docs)]
//! A utility to create a DNA file from a source working directory, and vice-versa
//!
//! This utility expects a working directory of the following structure:
//! ```sh
//! test-dna.dna.workdir/
//! ├── dna.yaml
//! ├── test-zome-1.wasm
//! └── test-zome-2.wasm
//! ```
//! Usage instructions from the `--help` flag:
//! ```sh
//! $ dna_util --help
//!
//!     dna_util 0.0.1
//!     Holochain DnaFile Utility.
//!
//!     USAGE:
//! dna_util [OPTIONS]
//!
//!     FLAGS:
//! -h, --help
//!     Prints help information
//!
//!     -V, --version
//!     Prints version information
//!
//!
//!     OPTIONS:
//! -c, --compile <compile>
//!     Compile a Dna Working Directory into a DnaFile.
//!
//!     (`dna_util -c my-dna.dna_work_dir` creates file `my-dna.dna.gz`)
//!     -e, --extract <extract>
//!     Extract a DnaFile into a Dna Working Directory.
//!
//!     (`dna_util -e my-dna.dna.gz` creates dir `my-dna.dna_work_dir`)
//! ```

use holochain_serialized_bytes::prelude::*;
use holochain_types::prelude::*;
use holochain_zome_types::zome::ZomeName;
use mr_bundle::error::MrBundleError;
use std::path::PathBuf;
use std::{collections::BTreeMap, path::Path};
use tokio::fs;

pub const BUNDLE_EXT: &str = ".dna";

/// DnaUtilError type.
#[derive(Debug, thiserror::Error)]
pub enum DnaUtilError {
    /// std::io::Error
    #[error("IO error: {0}")]
    StdIoError(#[from] std::io::Error),

    /// Missing filesystem path
    #[error("Couldn't find path: {1:?}. Detail: {0}")]
    PathNotFound(std::io::Error, PathBuf),

    /// DnaError
    #[error("DNA error: {0}")]
    DnaError(#[from] holochain_types::dna::DnaError),

    /// MrBundleError
    #[error(transparent)]
    MrBundleError(#[from] mr_bundle::error::MrBundleError),

    /// SerializedBytesError
    #[error("Internal serialization error: {0}")]
    SerializedBytesError(#[from] SerializedBytesError),

    /// serde_yaml::Error
    #[error("YAML serialization error: {0}")]
    SerdeYamlError(#[from] serde_yaml::Error),

    /// InvalidInput
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// anything else
    #[error("Unknown error: {0}")]
    MiscError(#[from] Box<dyn std::error::Error + Send + Sync>),

    #[error("This file should have a '{}' extension: {0}", BUNDLE_EXT)]
    FileExtensionMissing(PathBuf),
}

/// DnaUtil Result type.
pub type DnaUtilResult<T> = Result<T, DnaUtilError>;

/// Expand a DnaFile into a working directory
pub async fn expand(
    bundle_path: &impl AsRef<std::path::Path>,
    target_dir: Option<&Path>,
) -> DnaUtilResult<()> {
    let bundle_path = bundle_path.as_ref().canonicalize()?;
    let bundle: DnaBundle = mr_bundle::Bundle::read_from_file(&bundle_path)
        .await?
        .into();

    let target_dir = if let Some(d) = target_dir {
        d.to_owned()
    } else {
        bundle_path_to_dir(&bundle_path)?
    };

    bundle.explode_yaml(&target_dir).await?;

    Ok(())
}

fn bundle_path_to_dir(path: &Path) -> DnaUtilResult<PathBuf> {
    let bad_ext_err = || DnaUtilError::FileExtensionMissing(path.to_owned());
    let ext = path.extension().ok_or_else(bad_ext_err)?;
    if ext != BUNDLE_EXT {
        return Err(bad_ext_err());
    }
    let stem = path
        .file_stem()
        .expect("A file with an extension also has a stem");

    Ok(path
        .parent()
        .expect("file path should have parent")
        .join(stem))
}

/// Compress a Dna Working Directory into a DnaFile
pub async fn compress(
    manifest_path: &impl AsRef<std::path::Path>,
    target_path: Option<&Path>,
) -> DnaUtilResult<()> {
    let manifest_path = manifest_path.as_ref().canonicalize()?;
    let bundle: DnaBundle = mr_bundle::Bundle::implode_yaml(&manifest_path)
        .await?
        .into();
    let target_path = target_path.ok_or_else(|| bundle.find_root_dir(&manifest_path))?;
    bundle.write_to_file(target_path).await?;
    Ok(())
}

/// See `holochain_types::dna::zome::Zome`.
/// This is a helper to convert to yaml.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ZomeYaml {
    pub wasm_path: String,
}

/// Special Yaml Value Decode Helper
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
struct YamlValueDecodeHelper(pub serde_yaml::Value);

/// See `holochain_types::dna::DnaDef`.
/// This is a helper to convert to yaml.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct DnaDefYaml {
    pub name: String,
    pub uuid: String,
    pub properties: serde_yaml::Value,
    pub zomes: BTreeMap<ZomeName, ZomeYaml>,
}

impl DnaDefYaml {
    pub fn from_dna_def(dna: DnaDef) -> DnaUtilResult<DnaDefYaml> {
        let properties: YamlValueDecodeHelper = dna.properties.try_into()?;
        let mut zomes = BTreeMap::new();
        for zome_name in dna.zomes.into_iter().map(|(name, _)| name) {
            let zome_file = format!("./{}.wasm", zome_name);
            zomes.insert(
                zome_name.clone(),
                ZomeYaml {
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

    pub async fn compile_dna_file(
        &self,
        work_dir: impl Into<std::path::PathBuf>,
    ) -> DnaUtilResult<DnaFile> {
        let work_dir = work_dir.into();

        let properties: SerializedBytes =
            YamlValueDecodeHelper(self.properties.clone()).try_into()?;

        let mut zomes = Vec::new();
        let mut wasm_list = Vec::new();

        for (zome_name, zome) in self.zomes.iter() {
            let mut zome_file_path = work_dir.clone();
            zome_file_path.push(&zome.wasm_path);

            let zome_content = fs::read(zome_file_path).await?;

            let wasm: DnaWasm = zome_content.into();
            let wasm_hash = holo_hash::WasmHash::with_data(&wasm).await;
            zomes.push((zome_name.clone(), WasmZome { wasm_hash }.into()));
            wasm_list.push(wasm);
        }

        let dna = DnaDef {
            name: self.name.clone(),
            uuid: self.uuid.clone(),
            properties,
            zomes,
        };

        Ok(DnaFile::new(dna, wasm_list).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(threaded_scheduler)]
    async fn test_extract_then_compile() {
        let tmp_dir = tempdir::TempDir::new("dna_util_test").unwrap();

        let dna_file = holochain_types::test_utils::fake_dna_zomes(
            "bla",
            vec![
                ("test-zome-1".into(), vec![1, 2, 3, 4].into()),
                ("test-zome-2".into(), vec![5, 6, 7, 8].into()),
            ],
        );
        let properties = YamlValueDecodeHelper(
            serde_yaml::from_str(
                r#"
test_prop_1:
    - "a"
    - 42
test_prop_2:
    bool: true
        "#,
            )
            .unwrap(),
        );
        let dna_file = dna_file
            .with_properties(SerializedBytes::try_from(properties).unwrap())
            .await
            .unwrap();

        let dna_filename = tmp_dir.path().join("test-dna.dna.gz");
        let content1 = dna_file.to_file_content().await.unwrap();

        fs::write(&dna_filename, content1.clone()).await.unwrap();

        {
            let dna_file_path = dna_filename.as_path().canonicalize().unwrap();
            let dir = dna_file_path_convert(&dna_file_path, true).unwrap();
            fs::create_dir_all(&dir).await.unwrap();
            let content2 = fs::read(dna_file_path).await.unwrap();

            assert_eq!(content1, content2);
        };

        expand(&dna_filename).await.unwrap();

        fs::remove_file(&dna_filename).await.unwrap();

        compress(&tmp_dir.path().join("test-dna.dna.workdir"))
            .await
            .unwrap();

        let content = fs::read(&dna_filename).await.unwrap();
        let dna_file2 = DnaFile::from_file_content(&content).await.unwrap();

        assert_eq!(dna_file, dna_file2);
    }
}
