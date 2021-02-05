use std::collections::BTreeMap;

use super::{DnaDef, DnaFile, WasmMap};
use crate::prelude::*;
use holo_hash::*;

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
    /// Convert to a DnaFile
    pub async fn into_dna_file(self) -> DnaResult<DnaFile> {
        let (zomes, wasms) = self.inner_maps().await?;
        let dna_def = self.dna_def(zomes).await?;

        Ok(DnaFile::from_parts(dna_def, wasms))
    }

    async fn inner_maps(&self) -> DnaResult<(Zomes, WasmMap)> {
        let mut resources = self.resolve_all_cloned().await?;
        let intermediate: Vec<_> = self
            .manifest()
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
            .collect();

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
    pub async fn dna_def(&self, zomes: Zomes) -> DnaResult<DnaDefHashed> {
        let manifest = self.manifest();
        let properties: SerializedBytes = manifest
            .properties
            .as_ref()
            .map(|p| SerializedBytes::try_from(p))
            .unwrap_or_else(|| SerializedBytes::try_from(()))?;
        let dna_def = DnaDef {
            name: manifest.name.clone(),
            uuid: manifest.uuid.clone().unwrap_or_default(),
            properties,
            zomes,
        };

        Ok(DnaDefHashed::from_content(dna_def).await)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::prelude::ZomeManifest;

    #[tokio::test(threaded_scheduler)]
    async fn dna_bundle_to_dna_file() {
        let path1 = PathBuf::from("1");
        let path2 = PathBuf::from("2");
        let wasm1 = vec![1, 2, 3];
        let wasm2 = vec![4, 5, 6];
        let hash1 = WasmHash::with_data(&DnaWasm::from(wasm1.clone())).await;
        let hash2 = WasmHash::with_data(&DnaWasm::from(wasm2.clone())).await;
        let mut manifest = DnaManifest {
            name: "name".into(),
            uuid: None,
            properties: None,
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

        // Show that conversion fails due to hash mismatch
        let bad_bundle: DnaBundle =
            mr_bundle::Bundle::new_unchecked(manifest.clone(), resources.clone())
                .unwrap()
                .into();
        matches::assert_matches!(
            bad_bundle.into_dna_file().await,
            Err(DnaError::WasmHashMismatch(h1, h2))
            if h1 == hash1 && h2 == hash2
        );

        // Correct the hash and try again
        manifest.zomes[1].hash = Some(hash2.into());
        let bundle: DnaBundle = mr_bundle::Bundle::new_unchecked(manifest, resources)
            .unwrap()
            .into();
        let dna_file: DnaFile = bundle.into_dna_file().await.unwrap();
        assert_eq!(dna_file.dna_def().zomes.len(), 2);
        assert_eq!(dna_file.code().len(), 2);
    }
}
