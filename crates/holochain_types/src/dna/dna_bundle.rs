use crate::prelude::*;
use futures::StreamExt;
use holo_hash::*;
use mr_bundle::{Bundle, ResourceBytes, ResourceIdentifier};
use std::io::Read;
use std::{
    collections::{BTreeMap, HashMap},
};

#[cfg(test)]
mod test;

/// A bundle of Wasm zomes, represented as a file.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    SerializedBytes,
    shrinkwraprs::Shrinkwrap,
    derive_more::From,
)]
pub struct DnaBundle(Bundle<ValidatedDnaManifest>);

impl DnaBundle {
    /// Constructor
    pub fn new(
        manifest: ValidatedDnaManifest,
        resources: Vec<(ResourceIdentifier, ResourceBytes)>,
    ) -> DnaResult<Self> {
        Ok(Bundle::new(manifest, resources)?.into())
    }

    /// Convert to a DnaFile, and return what the hash of the Dna *would* have
    /// been without the provided modifier overrides
    pub async fn into_dna_file(
        self,
        override_modifiers: DnaModifiersOpt,
    ) -> DnaResult<(DnaFile, DnaHash)> {
        let (integrity, coordinator, wasms) = self.inner_maps().await?;
        let (dna_def, original_hash) =
            self.to_dna_def(integrity, coordinator, override_modifiers)?;

        Ok((
            DnaFile::new(dna_def.content, wasms.into_iter().map(|(_, v)| v)).await,
            original_hash,
        ))
    }

    /// Convert to a DnaFile without overriding modifiers
    pub async fn to_dna_file(self) -> DnaResult<(DnaFile, DnaHash)> {
        self.into_dna_file(DnaModifiersOpt::none()).await
    }

    /// Construct from raw bytes
    pub fn unpack(bytes: impl Read) -> DnaResult<Self> {
        Bundle::unpack(bytes).map(Into::into).map_err(Into::into)
    }

    async fn inner_maps(&self) -> DnaResult<(IntegrityZomes, CoordinatorZomes, WasmMap)> {
        let mut resources = self.get_all_resources().clone();
        let data = match &self.manifest().0 {
            DnaManifest::V1(manifest) => {
                let integrity =
                    hash_bytes(manifest.integrity.zomes.iter().cloned(), &mut resources).await?;
                let coordinator =
                    hash_bytes(manifest.coordinator.zomes.iter().cloned(), &mut resources).await?;
                [integrity, coordinator]
            }
        };

        let integrity_zomes = data[0]
            .iter()
            .map(|(zome_name, hash, _, dependencies)| {
                let zome_def = ZomeDef::Wasm(WasmZome {
                    wasm_hash: hash.clone(),
                    dependencies: dependencies.clone(),
                });
                (zome_name.clone(), zome_def.into())
            })
            .collect();
        let coordinator_zomes = data[1]
            .iter()
            .map(|(zome_name, hash, _, dependencies)| {
                let zome_def = ZomeDef::Wasm(WasmZome {
                    wasm_hash: hash.clone(),
                    dependencies: dependencies.clone(),
                });
                (zome_name.clone(), zome_def.into())
            })
            .collect();
        let code: BTreeMap<_, _> = data
            .into_iter()
            .flatten()
            .map(|(_, hash, wasm, _)| (hash, wasm))
            .collect();

        let wasms = WasmMap::from(code);

        Ok((integrity_zomes, coordinator_zomes, wasms))
    }

    /// Convert to a DnaDef
    pub fn to_dna_def(
        &self,
        integrity_zomes: IntegrityZomes,
        coordinator_zomes: CoordinatorZomes,
        modifiers: DnaModifiersOpt,
    ) -> DnaResult<(DnaDefHashed, DnaHash)> {
        match &self.manifest().0 {
            DnaManifest::V1(manifest) => {
                let dna_def = DnaDef {
                    name: manifest.name.clone(),
                    modifiers: DnaModifiers {
                        network_seed: manifest.integrity.network_seed.clone().unwrap_or_default(),
                        properties: SerializedBytes::try_from(
                            manifest.integrity.properties.clone().unwrap_or_default(),
                        )?,
                    },
                    integrity_zomes,
                    coordinator_zomes,
                    #[cfg(feature = "unstable-migration")]
                    lineage: manifest
                        .lineage
                        .clone()
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                };

                let original_hash = DnaHash::with_data_sync(&dna_def);
                let ddh = DnaDefHashed::from_content_sync(dna_def.update_modifiers(modifiers));
                Ok((ddh, original_hash))
            }
        }
    }

    /// Build a bundle from a DnaFile. Useful for tests.
    #[cfg(feature = "test_utils")]
    pub fn from_dna_file(dna_file: DnaFile) -> DnaResult<Self> {
        let DnaFile { ref dna, code, .. } = dna_file;
        let manifest = Self::manifest_from_dna_def(dna.clone().into_content())?;
        let resources = code
            .iter()
            .map(|(hash, wasm)| {
                let file_name = dna_file
                    .dna
                    .all_zomes()
                    .find_map(|(name, def)| match def {
                        ZomeDef::Wasm(w) if &w.wasm_hash == hash => Some(format!("{name}.wasm")),
                        _ => None,
                    })
                    .unwrap();
                (file_name, wasm.code.to_vec().into())
            })
            .collect();
        DnaBundle::new(manifest.try_into()?, resources)
    }

    #[cfg(feature = "test_utils")]
    fn manifest_from_dna_def(dna_def: DnaDef) -> DnaResult<DnaManifest> {
        let integrity = dna_def
            .integrity_zomes
            .into_iter()
            .filter_map(|(name, zome)| {
                let dependencies = zome
                    .as_any_zome_def()
                    .dependencies()
                    .iter()
                    .cloned()
                    .map(|name| ZomeDependency { name })
                    .collect();
                zome.wasm_hash(&name).ok().map(|hash| {
                    let hash = WasmHashB64::from(hash);
                    ZomeManifest {
                        name: name.clone(),
                        hash: Some(hash),
                        file: format!("{}.wasm", name),
                        dependencies: Some(dependencies),
                    }
                })
            })
            .collect();
        let coordinator = dna_def
            .coordinator_zomes
            .into_iter()
            .filter_map(|(name, zome)| {
                let dependencies = zome
                    .as_any_zome_def()
                    .dependencies()
                    .iter()
                    .cloned()
                    .map(|name| ZomeDependency { name })
                    .collect();
                zome.wasm_hash(&name).ok().map(|hash| {
                    let hash = WasmHashB64::from(hash);
                    ZomeManifest {
                        name: name.clone(),
                        hash: Some(hash),
                        file: format!("{}.wasm", name),
                        dependencies: Some(dependencies),
                    }
                })
            })
            .collect();
        #[cfg(feature = "unstable-migration")]
        let lineage = dna_def.lineage.into_iter().map(Into::into).collect();
        Ok(DnaManifestCurrent {
            name: dna_def.name,
            integrity: IntegrityManifest {
                network_seed: Some(dna_def.modifiers.network_seed),
                properties: Some(dna_def.modifiers.properties.try_into().map_err(|e| {
                    DnaError::DnaFileToBundleConversionError(format!(
                        "DnaDef properties were not YAML-deserializable: {}",
                        e
                    ))
                })?),
                zomes: integrity,
            },
            coordinator: CoordinatorManifest { zomes: coordinator },
            #[cfg(feature = "unstable-migration")]
            lineage,
        }
        .into())
    }
}

pub(super) async fn hash_bytes(
    zomes: impl Iterator<Item = ZomeManifest>,
    resources: &mut HashMap<&ResourceIdentifier, &ResourceBytes>,
) -> DnaResult<Vec<(ZomeName, WasmHash, DnaWasm, Vec<ZomeName>)>> {
    let iter = zomes.map(|z| {
        // println!("Have resources: {:?}", resources.keys());

        let bytes: bytes::Bytes = resources
            .remove(&z.resource_id())
            .expect(&format!("resource referenced in manifest must exist: {}", z.resource_id()))
            .clone()
            .into();
        let zome_name = z.name;
        let expected_hash = z.hash.map(WasmHash::from);
        let wasm = DnaWasm::from(bytes);
        let dependencies = z.dependencies.map_or(Vec::with_capacity(0), |deps| {
            deps.into_iter().map(|d| d.name).collect()
        });
        async move {
            let hash = wasm.to_hash().await;
            if let Some(expected) = expected_hash {
                if hash != expected {
                    return Err(DnaError::WasmHashMismatch(expected, hash));
                }
            }
            DnaResult::Ok((zome_name, hash, wasm, dependencies))
        }
    });
    futures::stream::iter(iter)
        .buffered(10)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn dna_bundle_to_dna_file() {
        let wasm1 = vec![1, 2, 3];
        let wasm2 = vec![4, 5, 6];
        let hash1 = DnaWasm::from(wasm1.clone()).to_hash().await;
        let hash2 = DnaWasm::from(wasm2.clone()).to_hash().await;
        #[cfg(feature = "unstable-migration")]
        let lineage = vec![DnaHash::from_raw_36(vec![11; 36]).into()];
        let mut manifest = DnaManifestCurrent {
            name: "name".into(),
            integrity: IntegrityManifest {
                network_seed: Some("original network seed".to_string()),
                properties: Some(serde_yaml::Value::Null.into()),
                zomes: vec![
                    ZomeManifest {
                        name: "zome1".into(),
                        hash: None,
                        file: "path1".to_string(),
                        dependencies: Default::default(),
                    },
                    ZomeManifest {
                        name: "zome2".into(),
                        // Intentional wrong hash
                        hash: Some(hash1.clone().into()),
                        file: "path2".to_string(),
                        dependencies: Default::default(),
                    },
                ],
            },
            coordinator: CoordinatorManifest { zomes: vec![] },
            #[cfg(feature = "unstable-migration")]
            lineage,
        };
        let resources = vec![
            ("path1".into(), wasm1.into()),
            ("path2".into(), wasm2.into()),
        ];

        // - Show that conversion fails due to hash mismatch
        let bad_bundle: DnaBundle =
            Bundle::new(manifest.clone().try_into().unwrap(), resources.clone())
                .unwrap()
                .into();
        matches::assert_matches!(
            bad_bundle.into_dna_file(DnaModifiersOpt::none()).await,
            Err(DnaError::WasmHashMismatch(h1, h2))
            if h1 == hash1 && h2 == hash2
        );

        // - Correct the hash and try again
        manifest.integrity.zomes[1].hash = Some(hash2.into());
        let bundle: DnaBundle =
            Bundle::new(manifest.clone().try_into().unwrap(), resources.clone())
                .unwrap()
                .into();
        let dna_file: DnaFile = bundle
            .into_dna_file(DnaModifiersOpt::none())
            .await
            .unwrap()
            .0;
        assert_eq!(dna_file.dna_def().integrity_zomes.len(), 2);
        assert_eq!(dna_file.code().len(), 2);

        // - Check that properties and UUID can be overridden
        let properties: YamlProperties = serde_yaml::Value::from(42).into();
        let bundle: DnaBundle = Bundle::new(manifest.try_into().unwrap(), resources)
            .unwrap()
            .into();
        let dna_file: DnaFile = bundle
            .into_dna_file(
                DnaModifiersOpt::none()
                    .with_network_seed("network_seed".into())
                    .with_properties(properties.clone())
                    .serialized()
                    .unwrap(),
            )
            .await
            .unwrap()
            .0;
        assert_eq!(
            dna_file.dna.modifiers.network_seed,
            "network_seed".to_string()
        );
        assert_eq!(
            dna_file.dna.modifiers.properties,
            SerializedBytes::try_from(properties).unwrap()
        );
    }
}
