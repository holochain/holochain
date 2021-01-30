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

use crate::error::{HcBundleError, HcBundleResult};
use holochain_serialized_bytes::prelude::*;
use holochain_types::prelude::*;
use holochain_zome_types::zome::ZomeName;
use std::path::PathBuf;
use std::{collections::BTreeMap, path::Path};
use tokio::fs;

/// The file extension to use for DNA bundles
pub const DNA_BUNDLE_EXT: &str = "dna";

/// Unpack a DNA bundle into a working directory, returning the directory path used.
pub async fn unpack(
    bundle_path: &impl AsRef<std::path::Path>,
    target_dir: Option<PathBuf>,
    force: bool,
) -> HcBundleResult<PathBuf> {
    let bundle_path = bundle_path.as_ref().canonicalize()?;
    let bundle: DnaBundle = mr_bundle::Bundle::read_from_file(&bundle_path)
        .await?
        .into();

    let target_dir = if let Some(d) = target_dir {
        d
    } else {
        bundle_path_to_dir(&bundle_path)?
    };

    bundle.unpack_yaml(&target_dir, force).await?;

    Ok(target_dir)
}

fn bundle_path_to_dir(path: &Path) -> HcBundleResult<PathBuf> {
    let bad_ext_err = || HcBundleError::FileExtensionMissing(path.to_owned());
    let ext = path.extension().ok_or_else(bad_ext_err)?;
    if ext != DNA_BUNDLE_EXT {
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

/// Pack a directory containing a DNA manifest into a DnaBundle, returning
/// the path to which the bundle file was written
pub async fn pack(
    dir_path: &impl AsRef<std::path::Path>,
    target_path: Option<PathBuf>,
) -> HcBundleResult<(PathBuf, DnaBundle)> {
    let dir_path = dir_path.as_ref().canonicalize()?;
    let manifest_path = dir_path.join(&DnaManifest::relative_path());
    let bundle: DnaBundle = mr_bundle::Bundle::pack_yaml(&manifest_path).await?.into();
    let target_path = target_path
        .map(Ok)
        .unwrap_or_else(|| dir_to_bundle_path(&dir_path))?;
    bundle.write_to_file(&target_path).await?;
    Ok((target_path, bundle))
}

fn dir_to_bundle_path(dir_path: &Path) -> HcBundleResult<PathBuf> {
    let dir_name = dir_path.file_name().expect("Cannot pack `/`");
    let parent_path = dir_path.parent().expect("Cannot pack `/`");
    Ok(parent_path.join(format!("{}.dna", dir_name.to_string_lossy())))
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
    pub fn from_dna_def(dna: DnaDef) -> HcBundleResult<DnaDefYaml> {
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
    ) -> HcBundleResult<DnaFile> {
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
    use mr_bundle::error::{MrBundleError, UnpackingError};

    use super::*;

    #[tokio::test(threaded_scheduler)]
    async fn test_roundtrip() {
        let tmpdir = tempdir::TempDir::new("hc-bundle-test").unwrap();
        let dir = tmpdir.path().join("test-dna");
        std::fs::create_dir(&dir).unwrap();

        let manifest_yaml = r#"
---
name: test dna
uuid: blablabla
properties:
  some: 42
  props: yay
zomes:
  - name: zome1
    bundled: zome-1.wasm
  - name: zome2
    bundled: nested/zome-2.wasm
  - name: zome3
    path: ../zome-3.wasm
        "#;

        // Create files in working directory
        std::fs::create_dir(dir.join("nested")).unwrap();
        std::fs::write(dir.join("zome-1.wasm"), &[1, 2, 3]).unwrap();
        std::fs::write(dir.join("nested/zome-2.wasm"), &[4, 5, 6]).unwrap();
        std::fs::write(dir.join("dna.yaml"), manifest_yaml.as_bytes()).unwrap();

        // Create a local file that's not actually part of the bundle,
        // in the parent directory
        std::fs::write(tmpdir.path().join("zome-3.wasm"), &[7, 8, 9]).unwrap();

        let (bundle_path, bundle) = pack(&dir, None).await.unwrap();

        // Ensure the bundle path was generated as expected
        assert!(bundle_path.is_file());
        assert_eq!(bundle_path.parent(), dir.parent());
        assert_eq!(bundle_path, dir.parent().unwrap().join("test-dna.dna"));

        // Ensure we can resolve all files, including the local one
        assert_eq!(bundle.resolve_all().await.unwrap().values().len(), 3);

        // Unpack without forcing, which will fail
        matches::assert_matches!(
            unpack(&bundle_path, None, false).await,
            Err(
                HcBundleError::MrBundleError(
                    MrBundleError::UnpackingError(UnpackingError::DirectoryExists(_)),
                ),
            )
        );
        // Now unpack with forcing to overwrite original directory
        unpack(&bundle_path, None, true).await.unwrap();

        // Now remove the directory altogether, unpack again, and check that
        // all of the same files are present
        std::fs::remove_dir_all(&dir).unwrap();
        unpack(&bundle_path, None, false).await.unwrap();
        assert!(dir.join("zome-1.wasm").is_file());
        assert!(dir.join("nested/zome-2.wasm").is_file());
        assert!(dir.join("dna.yaml").is_file());

        // Ensure that these are the only 3 files
        assert_eq!(dir.read_dir().unwrap().collect::<Vec<_>>().len(), 3);

        // Ensure that we get the same bundle after the roundtrip
        let (_, bundle2) = pack(&dir, None).await.unwrap();
        assert_eq!(bundle, bundle2);
    }
}
