use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::CoordinatorZomes;
use holochain_zome_types::WasmZome;
use holochain_zome_types::ZomeDef;
use mr_bundle::Manifest;

use crate::prelude::DnaResult;
use crate::prelude::DnaWasm;

use super::hash_bytes;
use super::CoordinatorManifest;

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
/// A bundle of coordinator zomes.
pub struct CoordinatorBundle(mr_bundle::Bundle<CoordinatorManifest>);

impl Manifest for CoordinatorManifest {
    fn locations(&self) -> Vec<mr_bundle::Location> {
        self.zomes
            .iter()
            .map(|zome| zome.location.clone())
            .collect()
    }

    fn path() -> std::path::PathBuf {
        "coordinators.yaml".into()
    }

    fn bundle_extension() -> &'static str {
        "coordinators"
    }
}

impl CoordinatorBundle {
    /// Convert into zomes and their wasm files.
    pub async fn into_zomes(self) -> DnaResult<(CoordinatorZomes, Vec<DnaWasm>)> {
        let mut resources = self.resolve_all_cloned().await?;
        let coordinator = hash_bytes(self.manifest().zomes.iter().cloned(), &mut resources).await?;
        let coordinator_zomes = coordinator
            .iter()
            .map(|(zome_name, hash, _, dependencies)| {
                (
                    zome_name.clone(),
                    ZomeDef::Wasm(WasmZome {
                        wasm_hash: hash.clone(),
                        dependencies: dependencies.clone(),
                    })
                    .into(),
                )
            })
            .collect();
        let wasms = coordinator
            .into_iter()
            .map(|(_, _, wasm, _)| wasm)
            .collect();

        Ok((coordinator_zomes, wasms))
    }
}
