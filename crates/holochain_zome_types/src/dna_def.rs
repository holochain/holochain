//! Defines DnaDef struct

use super::zome;
use crate::prelude::*;

#[cfg(feature = "full-dna-def")]
use crate::zome::error::ZomeError;
#[cfg(feature = "full-dna-def")]
use holo_hash::*;

/// Zomes need to be an ordered map from ZomeName to a Zome
pub type Zomes = Vec<(ZomeName, zome::ZomeDef)>;

/// Placeholder for a real UID type
pub type Uid = String;

/// The definition of a DNA: the hash of this data is what produces the DnaHash.
///
/// Historical note: This struct was written before `DnaManifest` appeared.
/// It is included as part of a `DnaFile`. There is still a lot of code that uses
/// this type, but in function, it has mainly been superseded by `DnaManifest`.
/// Hence, this type can basically be thought of as a fully validated, normalized
/// `DnaManifest`
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, SerializedBytes)]
#[cfg_attr(feature = "full-dna-def", derive(derive_builder::Builder))]
#[cfg_attr(feature = "full-dna-def", builder(public))]
pub struct DnaDef {
    /// The friendly "name" of a Holochain DNA.
    #[cfg_attr(
        feature = "full-dna-def",
        builder(default = "\"Generated DnaDef\".to_string()")
    )]
    pub name: String,

    /// A UID for uniquifying this Dna.
    // TODO: consider Vec<u8> instead (https://github.com/holochain/holochain/pull/86#discussion_r412689085)
    pub uid: String,

    /// Any arbitrary application properties can be included in this object.
    #[cfg_attr(feature = "full-dna-def", builder(default = "().try_into().unwrap()"))]
    pub properties: SerializedBytes,

    /// The time used to denote the origin of the network, used to calculate
    /// time windows during gossip.
    /// All Header timestamps must come after this time.
    #[cfg_attr(feature = "full-dna-def", builder(default = "Timestamp::now()"))]
    pub origin_time: Timestamp,

    /// A vector of zomes associated with your DNA.
    pub zomes: Zomes,
}

#[cfg(feature = "test_utils")]
impl DnaDef {
    /// Create a DnaDef with a random UID, useful for testing
    pub fn unique_from_zomes(zomes: Vec<Zome>) -> DnaDef {
        let zomes = zomes.into_iter().map(|z| z.into_inner()).collect();
        DnaDefBuilder::default()
            .zomes(zomes)
            .random_uid()
            .build()
            .unwrap()
    }
}

#[cfg(feature = "full-dna-def")]
impl DnaDef {
    /// Return a Zome
    pub fn get_zome(&self, zome_name: &ZomeName) -> Result<zome::Zome, ZomeError> {
        self.zomes
            .iter()
            .find(|(name, _)| name == zome_name)
            .cloned()
            .map(|(name, def)| Zome::new(name, def))
            .ok_or_else(|| ZomeError::ZomeNotFound(format!("Zome '{}' not found", &zome_name,)))
    }

    /// Return a Zome by its index
    pub fn get_zome_by_index(&self, zome_id: &ZomeId) -> Result<zome::Zome, ZomeError> {
        self.zomes
            .get(zome_id.0 as usize)
            .cloned()
            .map(|(name, def)| Zome::new(name, def))
            .ok_or_else(|| ZomeError::ZomeNotFound(format!("Zome at index {} not found", zome_id)))
    }

    /// Return a Zome, error if not a WasmZome
    pub fn get_wasm_zome(&self, zome_name: &ZomeName) -> Result<&zome::WasmZome, ZomeError> {
        self.zomes
            .iter()
            .find(|(name, _)| name == zome_name)
            .map(|(_, def)| def)
            .ok_or_else(|| ZomeError::ZomeNotFound(format!("Zome '{}' not found", &zome_name,)))
            .and_then(|def| {
                if let ZomeDef::Wasm(wasm_zome) = def {
                    Ok(wasm_zome)
                } else {
                    Err(ZomeError::NonWasmZome(zome_name.clone()))
                }
            })
    }

    /// Change the "phenotype" of this DNA -- the UID and properties -- while
    /// leaving the "genotype" of actual DNA code intact
    pub fn modify_phenotype(&self, uid: Uid, properties: SerializedBytes) -> Self {
        let mut clone = self.clone();
        clone.properties = properties;
        clone.uid = uid;
        clone
    }
}

/// Get a random UID
#[cfg(feature = "full-dna-def")]
pub fn random_uid() -> String {
    nanoid::nanoid!()
}

#[cfg(feature = "full-dna-def")]
impl DnaDefBuilder {
    /// Provide a random UID
    pub fn random_uid(&mut self) -> &mut Self {
        self.uid = Some(random_uid());
        self
    }
}

/// A DnaDef paired with its DnaHash
#[cfg(feature = "full-dna-def")]
pub type DnaDefHashed = HoloHashed<DnaDef>;

#[cfg(feature = "full-dna-def")]
impl_hashable_content!(DnaDef, Dna);
