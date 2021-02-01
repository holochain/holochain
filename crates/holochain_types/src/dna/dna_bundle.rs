use std::collections::BTreeMap;

use super::{DnaDef, DnaFile, DnaManifest, WasmMap};
use crate::prelude::*;
use holo_hash::*;
use mr_bundle::error::MrBundleResult;

/// A bundle of Wasm zomes, respresented as a file.
#[derive(
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
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

    async fn inner_maps(&self) -> MrBundleResult<(Zomes, WasmMap)> {
        let mut resources = self.resolve_all_cloned().await?;
        let names_and_wasms: Vec<_> = self
            .manifest()
            .zomes
            .iter()
            .map(|z| {
                let bytes = resources
                    .remove(&z.location)
                    .expect("resource referenced in manifest must exist");
                (z.name.clone(), DnaWasm::from(bytes))
            })
            .collect();

        let data: Vec<_> =
            futures::future::join_all(names_and_wasms.into_iter().map(|(zome_name, wasm)| async {
                let hash = WasmHash::with_data(&wasm).await;
                (zome_name, hash, wasm)
            }))
            .await;

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
