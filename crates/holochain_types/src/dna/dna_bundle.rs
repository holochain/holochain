use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::prelude::*;
use holo_hash::*;
use mr_bundle::Location;

/// A bundle of Wasm zomes, respresented as a file.
#[derive(
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    SerializedBytes,
    shrinkwraprs::Shrinkwrap,
    derive_more::From,
)]
pub struct DnaBundle(mr_bundle::Bundle<DnaManifest>);

impl DnaBundle {
    /// Constructor
    pub fn new(
        manifest: DnaManifest,
        resources: Vec<(PathBuf, Vec<u8>)>,
        root_dir: PathBuf,
    ) -> DnaResult<Self> {
        Ok(mr_bundle::Bundle::new(manifest, resources, root_dir)?.into())
    }

    /// Convert to a DnaFile, and return what the hash of the Dna *would* have
    /// been without the provided phenotype overrides
    pub async fn into_dna_file(
        self,
        uid: Option<Uid>,
        properties: Option<YamlProperties>,
    ) -> DnaResult<(DnaFile, DnaHash)> {
        let (zomes, wasms) = self.inner_maps().await?;
        let (dna_def, original_hash) = self.to_dna_def(zomes, uid, properties)?;

        Ok((DnaFile::from_parts(dna_def, wasms), original_hash))
    }

    /// Construct from raw bytes
    pub fn decode(bytes: &[u8]) -> DnaResult<Self> {
        mr_bundle::Bundle::decode(bytes)
            .map(Into::into)
            .map_err(Into::into)
    }

    /// Read from a bundle file
    pub async fn read_from_file(path: &Path) -> DnaResult<Self> {
        mr_bundle::Bundle::read_from_file(path)
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    async fn inner_maps(&self) -> DnaResult<(Zomes, WasmMap)> {
        let mut resources = self.resolve_all_cloned().await?;
        let intermediate: Vec<_> = match self.manifest() {
            DnaManifest::V1(manifest) => manifest
                .zomes
                .iter()
                .map(|z| {
                    let bytes = resources
                        .remove(&z.location)
                        .expect("resource referenced in manifest must exist");
                    (
                        z.name.clone(),
                        z.hash.clone().map(WasmHash::from),
                        DnaWasm::from(bytes),
                    )
                })
                .collect(),
        };

        let data = futures::future::join_all(intermediate.into_iter().map(
            |(zome_name, expected_hash, wasm)| async {
                let hash = WasmHash::with_data(&wasm).await;
                if let Some(expected) = expected_hash {
                    if hash != expected {
                        return Err(DnaError::WasmHashMismatch(expected, hash));
                    }
                }
                DnaResult::Ok((zome_name, hash, wasm))
            },
        ))
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

        let zomes: Zomes = data
            .iter()
            .map(|(zome_name, hash, _)| {
                (
                    zome_name.clone(),
                    ZomeDef::Wasm(WasmZome::new(hash.clone())),
                )
            })
            .collect();

        let code: BTreeMap<_, _> = data
            .into_iter()
            .map(|(_, hash, wasm)| (hash, wasm))
            .into_iter()
            .collect();
        let wasms = WasmMap::from(code);

        Ok((zomes, wasms))
    }

    /// Convert to a DnaDef
    pub fn to_dna_def(
        &self,
        zomes: Zomes,
        uid: Option<Uid>,
        properties: Option<YamlProperties>,
    ) -> DnaResult<(DnaDefHashed, DnaHash)> {
        match self.manifest() {
            DnaManifest::V1(manifest) => {
                let mut dna_def = DnaDef {
                    name: manifest.name.clone(),
                    uid: manifest.uid.clone().unwrap_or_default(),
                    properties: SerializedBytes::try_from(
                        manifest.properties.clone().unwrap_or_default(),
                    )?,
                    zomes,
                };

                if uid.is_none() && properties.is_none() {
                    // If no phenotype overrides, then the original hash is the same as the current hash
                    let ddh = DnaDefHashed::from_content_sync(dna_def);
                    let original_hash = ddh.as_hash().clone();
                    Ok((ddh, original_hash))
                } else {
                    // Otherwise, record the original hash first, for version comparisons.
                    let original_hash = DnaHash::with_data_sync(&dna_def);

                    let properties: SerializedBytes = properties
                        .as_ref()
                        .or_else(|| manifest.properties.as_ref())
                        .map(SerializedBytes::try_from)
                        .unwrap_or_else(|| SerializedBytes::try_from(()))?;
                    let uid = uid.or_else(|| manifest.uid.clone()).unwrap_or_default();

                    dna_def.uid = uid;
                    dna_def.properties = properties;
                    Ok((DnaDefHashed::from_content_sync(dna_def), original_hash))
                }
            }
        }
    }

    /// Build a bundle from a DnaFile. Useful for tests.
    #[cfg(feature = "test_utils")]
    pub async fn from_dna_file(dna_file: DnaFile) -> DnaResult<Self> {
        let DnaFile { dna, code } = dna_file;
        let manifest = Self::manifest_from_dna_def(dna.into_content())?;
        let resources = code
            .into_iter()
            .map(|(hash, wasm)| (PathBuf::from(hash.to_string()), wasm.code.to_vec()))
            .collect();
        DnaBundle::new(manifest, resources, PathBuf::from("."))
    }

    #[cfg(feature = "test_utils")]
    fn manifest_from_dna_def(dna_def: DnaDef) -> DnaResult<DnaManifest> {
        let zomes = dna_def
            .zomes
            .into_iter()
            .filter_map(|(name, zome)| {
                match zome {
                    ZomeDef::Wasm(wz) => Some(wz.wasm_hash),
                    ZomeDef::Inline(_) => None,
                }
                .map(|hash| {
                    let hash = WasmHashB64::from(hash);
                    let filename = format!("{}", hash);
                    ZomeManifest {
                        name,
                        hash: Some(hash),
                        location: Location::Bundled(PathBuf::from(filename)),
                    }
                })
            })
            .collect();
        Ok(DnaManifestCurrent {
            name: dna_def.name,
            uid: Some(dna_def.uid),
            properties: Some(dna_def.properties.try_into().map_err(|e| {
                DnaError::DnaFileToBundleConversionError(format!(
                    "DnaDef properties were not YAML-deserializable: {}",
                    e
                ))
            })?),
            zomes,
        }
        .into())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn dna_bundle_to_dna_file() {
        let path1 = PathBuf::from("1");
        let path2 = PathBuf::from("2");
        let wasm1 = vec![1, 2, 3];
        let wasm2 = vec![4, 5, 6];
        let hash1 = WasmHash::with_data(&DnaWasm::from(wasm1.clone())).await;
        let hash2 = WasmHash::with_data(&DnaWasm::from(wasm2.clone())).await;
        let mut manifest = DnaManifestCurrent {
            name: "name".into(),
            uid: Some("original uid".to_string()),
            properties: Some(serde_yaml::Value::Null.into()),
            zomes: vec![
                ZomeManifest {
                    name: "zome1".into(),
                    hash: None,
                    location: mr_bundle::Location::Bundled(path1.clone()),
                },
                ZomeManifest {
                    name: "zome2".into(),
                    // Intentional wrong hash
                    hash: Some(hash1.clone().into()),
                    location: mr_bundle::Location::Bundled(path2.clone()),
                },
            ],
        };
        let resources = vec![(path1, wasm1), (path2, wasm2)];

        // - Show that conversion fails due to hash mismatch
        let bad_bundle: DnaBundle =
            mr_bundle::Bundle::new_unchecked(manifest.clone().into(), resources.clone())
                .unwrap()
                .into();
        matches::assert_matches!(
            bad_bundle.into_dna_file(None, None).await,
            Err(DnaError::WasmHashMismatch(h1, h2))
            if h1 == hash1 && h2 == hash2
        );

        // - Correct the hash and try again
        manifest.zomes[1].hash = Some(hash2.into());
        let bundle: DnaBundle =
            mr_bundle::Bundle::new_unchecked(manifest.clone().into(), resources.clone())
                .unwrap()
                .into();
        let dna_file: DnaFile = bundle.into_dna_file(None, None).await.unwrap().0;
        assert_eq!(dna_file.dna_def().zomes.len(), 2);
        assert_eq!(dna_file.code().len(), 2);

        // - Check that properties and UUID can be overridden
        let properties: YamlProperties = serde_yaml::Value::from(42).into();
        let bundle: DnaBundle = mr_bundle::Bundle::new_unchecked(manifest.into(), resources)
            .unwrap()
            .into();
        let dna_file: DnaFile = bundle
            .into_dna_file(Some("uid".into()), Some(properties.clone()))
            .await
            .unwrap()
            .0;
        assert_eq!(dna_file.dna.uid, "uid".to_string());
        assert_eq!(
            dna_file.dna.properties,
            SerializedBytes::try_from(properties).unwrap()
        );
    }
}
