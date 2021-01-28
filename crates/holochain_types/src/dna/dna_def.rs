use super::error::DnaError;
use super::zome;
use crate::prelude::*;
use holo_hash::*;
use holochain_zome_types::ZomeName;

/// Zomes need to be an ordered map from ZomeName to a Zome
pub type Zomes = Vec<(ZomeName, zome::ZomeDef)>;

/// A type to allow json values to be used as [SerializedBytes]
#[derive(Debug, Clone, derive_more::From, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct JsonProperties(serde_json::Value);

impl JsonProperties {
    /// Create new properties from json value
    pub fn new(properties: serde_json::Value) -> Self {
        JsonProperties(properties)
    }
}

/// Represents the top-level holochain dna object.
#[derive(
    Serialize, Deserialize, Clone, Debug, PartialEq, Eq, SerializedBytes, derive_builder::Builder,
)]
#[builder(public)]
pub struct DnaDef {
    /// The friendly "name" of a Holochain DNA.
    #[builder(default = "\"Generated DnaDef\".to_string()")]
    pub name: String,

    /// A UUID for uniquifying this Dna.
    // TODO: consider Vec<u8> instead (https://github.com/holochain/holochain/pull/86#discussion_r412689085)
    pub uuid: String,

    /// Any arbitrary application properties can be included in this object.
    #[builder(default = "().try_into().unwrap()")]
    pub properties: SerializedBytes,

    /// An array of zomes associated with your holochain application.
    pub zomes: Zomes,
}

#[cfg(feature = "test_utils")]
impl DnaDef {
    /// Create a DnaDef with a random UUID, useful for testing
    pub fn unique_from_zomes(zomes: Vec<Zome>) -> Self {
        let zomes = zomes.into_iter().map(|z| z.into_inner()).collect();
        DnaDefBuilder::default()
            .zomes(zomes)
            .random_uuid()
            .build()
            .unwrap()
    }
}

impl DnaDef {
    /// Return a Zome
    pub fn get_zome(&self, zome_name: &ZomeName) -> Result<zome::Zome, DnaError> {
        self.zomes
            .iter()
            .find(|(name, _)| name == zome_name)
            .cloned()
            .map(|(name, def)| Zome::new(name, def))
            .ok_or_else(|| DnaError::ZomeNotFound(format!("Zome '{}' not found", &zome_name,)))
    }

    /// Return a Zome, error if not a WasmZome
    pub fn get_wasm_zome(&self, zome_name: &ZomeName) -> Result<&zome::WasmZome, DnaError> {
        self.zomes
            .iter()
            .find(|(name, _)| name == zome_name)
            .map(|(_, def)| def)
            .ok_or_else(|| DnaError::ZomeNotFound(format!("Zome '{}' not found", &zome_name,)))
            .and_then(|def| {
                if let ZomeDef::Wasm(wasm_zome) = def {
                    Ok(wasm_zome)
                } else {
                    Err(DnaError::NonWasmZome(zome_name.clone()))
                }
            })
    }
}

fn random_uuid() -> String {
    nanoid::nanoid!()
}

impl DnaDefBuilder {
    /// Provide a random UUID
    pub fn random_uuid(&mut self) -> &mut Self {
        self.uuid = Some(random_uuid());
        self
    }
}

/// A DnaDef paired with its DnaHash
pub type DnaDefHashed = HoloHashed<DnaDef>;

impl_hashable_content!(DnaDef, Dna);
