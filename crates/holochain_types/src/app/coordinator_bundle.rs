use crate::app::app_manifest_v0::CoordinatorManifest;
use crate::dna::hash_bytes;
use crate::prelude::DnaResult;
use crate::prelude::DnaWasm;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::prelude::*;

/// A bundle of coordinator zomes.
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
pub struct CoordinatorBundle(mr_bundle::Bundle<CoordinatorManifest>);

impl CoordinatorBundle {
    /// Convert into zomes and their wasm files.
    pub async fn into_zomes(self) -> DnaResult<(CoordinatorZomes, Vec<DnaWasm>)> {
        let mut resources = self.get_all_resources().clone();
        let coordinator = hash_bytes(self.manifest().zomes.iter().cloned(), &mut resources).await?;
        let coordinator_zomes = coordinator
            .iter()
            .map(|(zome_name, hash, _, dependencies)| {
                let zome_def = ZomeDef::Wasm(WasmZome {
                    wasm_hash: hash.clone(),
                    dependencies: dependencies.clone(),
                });
                (zome_name.clone(), zome_def.into())
            })
            .collect();
        let wasms = coordinator
            .into_iter()
            .map(|(_, _, wasm, _)| wasm)
            .collect();

        Ok((coordinator_zomes, wasms))
    }
}
