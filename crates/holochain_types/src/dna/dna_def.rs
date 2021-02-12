use super::error::DnaError;
use super::zome;
use crate::prelude::*;
use holo_hash::*;
use holochain_zome_types::ZomeName;

/// Zomes need to be an ordered map from ZomeName to a Zome
pub type Zomes = Vec<(ZomeName, zome::ZomeDef)>;

/// A type to allow json values to be used as [SerializedBytes]
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    derive_more::From,
    serde::Serialize,
    serde::Deserialize,
    SerializedBytes,
)]
pub struct YamlProperties(serde_yaml::Value);

impl YamlProperties {
    /// Create new properties from json value
    pub fn new(properties: serde_yaml::Value) -> Self {
        Self(properties)
    }

    /// Create a null set of properties
    pub fn empty() -> Self {
        Self(serde_yaml::Value::Null)
    }
}

impl From<()> for YamlProperties {
    fn from(_: ()) -> Self {
        Self::empty()
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

    /// An array of zomes associated with your DNA.
    pub zomes: Zomes,
}

#[cfg(feature = "test_utils")]
impl DnaDef {
    /// Create a DnaDef with a random UUID, useful for testing
    pub fn unique_from_zomes(zomes: Vec<Zome>) -> DnaDef {
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

    /// Change the "phenotype" of this DNA -- the UUID and properties -- while
    /// leaving the "genotype" of actual DNA code intact
    pub fn modify_phenotype(&self, uuid: Uuid, properties: YamlProperties) -> DnaResult<Self> {
        let mut clone = self.clone();
        clone.properties = properties.try_into()?;
        clone.uuid = uuid;
        Ok(clone)
    }
}

/// Get a random UUID
pub fn random_uuid() -> String {
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
