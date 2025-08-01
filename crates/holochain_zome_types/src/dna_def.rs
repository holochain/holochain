//! Defines DnaDef struct

#[cfg(feature = "unstable-migration")]
use std::collections::HashSet;

use crate::prelude::*;

#[cfg(feature = "full-dna-def")]
use holochain_integrity_types::DnaModifiersBuilder;

#[cfg(feature = "full-dna-def")]
use crate::zome::ZomeError;
#[cfg(feature = "full-dna-def")]
use holo_hash::*;

/// Ordered list of integrity zomes in this DNA.
pub type IntegrityZomes = Vec<(ZomeName, IntegrityZomeDef)>;

/// Ordered list of coordinator zomes in this DNA.
pub type CoordinatorZomes = Vec<(ZomeName, CoordinatorZomeDef)>;

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

    /// Modifiers of this DNA - the network seed, properties - as opposed to
    /// the actual DNA code. The modifiers are included in the DNA hash
    /// computation.
    pub modifiers: DnaModifiers,

    /// A vector of zomes associated with your DNA.
    pub integrity_zomes: IntegrityZomes,

    /// A vector of zomes that do not affect
    /// the [`DnaHash`].
    pub coordinator_zomes: CoordinatorZomes,

    /// A list of past "ancestors" of this DNA.
    ///
    /// Whenever a DNA is created which is intended to be used as a migration from
    /// a previous DNA, the lineage should be updated to include the hash of the
    /// DNA being migrated from. DNA hashes may also be removed from this list if
    /// it is desired to remove them from the lineage.
    ///
    /// The meaning of the "ancestor" relationship is as follows:
    /// - For any DNA, there is a migration path from any of its ancestors to itself.
    /// - When an app depends on a DnaHash via UseExisting, it means that any installed
    ///   DNA in the lineage which contains that DnaHash can be used.
    /// - The app's Coordinator interface is expected to be compatible across the lineage.
    ///   (Though this cannot be enforced, since Coordinators can be swapped out at
    ///   will by the user, the intention is still there.)
    ///
    /// Holochain does nothing to ensure the correctness of the lineage, it is up to
    /// the app developer to make the necessary guarantees.
    #[cfg(feature = "unstable-migration")]
    #[serde(default)]
    #[cfg_attr(feature = "full-dna-def", builder(default))]
    pub lineage: HashSet<DnaHash>,
}

#[cfg(feature = "full-dna-def")]
#[derive(Serialize, Debug, PartialEq, Eq)]
/// A reference to for creating the hash for [`DnaDef`].
struct DnaDefHash<'a> {
    modifiers: &'a DnaModifiers,
    integrity_zomes: &'a IntegrityZomes,
}

#[cfg(feature = "test_utils")]
impl DnaDef {
    /// Create a DnaDef with a random network seed, useful for testing
    pub fn unique_from_zomes(
        integrity: Vec<IntegrityZome>,
        coordinator: Vec<CoordinatorZome>,
    ) -> DnaDef {
        let integrity = integrity.into_iter().map(|z| z.into_inner()).collect();
        let coordinator = coordinator.into_iter().map(|z| z.into_inner()).collect();
        DnaDefBuilder::default()
            .integrity_zomes(integrity)
            .coordinator_zomes(coordinator)
            .random_network_seed()
            .build()
            .unwrap()
    }
}

impl DnaDef {
    /// Get all zomes including the integrity and coordinator zomes.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    pub fn all_zomes(&self) -> impl Iterator<Item = (&ZomeName, &ZomeDef)> {
        self.integrity_zomes
            .iter()
            .map(|(n, def)| (n, def.as_any_zome_def()))
            .chain(
                self.coordinator_zomes
                    .iter()
                    .map(|(n, def)| (n, def.as_any_zome_def())),
            )
    }
}

#[cfg(feature = "full-dna-def")]
impl DnaDef {
    /// Find an integrity zome from a [`ZomeName`].
    pub fn get_integrity_zome(&self, zome_name: &ZomeName) -> Result<IntegrityZome, ZomeError> {
        self.integrity_zomes
            .iter()
            .find(|(name, _)| name == zome_name)
            .cloned()
            .map(|(name, def)| IntegrityZome::new(name, def))
            .ok_or_else(|| {
                tracing::error!(
                    "ZomeNotFound: {zome_name}. (get_integrity_zome) Existing zomes: integrity={:?}, coordinator={:?}",
                    self.integrity_zomes,
                    self.coordinator_zomes,
                );
                ZomeError::ZomeNotFound(format!("Integrity zome '{}' not found", &zome_name,))
            })
    }

    /// Check if a zome is an integrity zome.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    pub fn is_integrity_zome(&self, zome_name: &ZomeName) -> bool {
        self.integrity_zomes
            .iter()
            .any(|(name, _)| name == zome_name)
    }

    /// Find a coordinator zome from a [`ZomeName`].
    pub fn get_coordinator_zome(&self, zome_name: &ZomeName) -> Result<CoordinatorZome, ZomeError> {
        self.coordinator_zomes
            .iter()
            .find(|(name, _)| name == zome_name)
            .cloned()
            .map(|(name, def)| CoordinatorZome::new(name, def))
            .ok_or_else(|| {
                tracing::error!(
                    "ZomeNotFound: {zome_name}. (get_coordinator_zome) Existing zomes: integrity={:?}, coordinator={:?}",
                    self.integrity_zomes,
                    self.coordinator_zomes,
                );
                ZomeError::ZomeNotFound(format!("Coordinator Zome '{}' not found", &zome_name,))
            })
    }

    /// Find a any zome from a [`ZomeName`].
    pub fn get_zome(&self, zome_name: &ZomeName) -> Result<Zome, ZomeError> {
        self.integrity_zomes
            .iter()
            .find(|(name, _)| name == zome_name)
            .cloned()
            .map(|(name, def)| Zome::new(name, def.erase_type()))
            .or_else(|| {
                self.coordinator_zomes
                    .iter()
                    .find(|(name, _)| name == zome_name)
                    .cloned()
                    .map(|(name, def)| Zome::new(name, def.erase_type()))
            })
            .ok_or_else(|| {
                tracing::error!(
                    "ZomeNotFound: {zome_name}. (get_zome) Existing zomes: integrity={:?}, coordinator={:?}",
                    self.integrity_zomes,
                    self.coordinator_zomes,
                );
                ZomeError::ZomeNotFound(format!("Zome '{}' not found", &zome_name,))
            })
    }

    /// Get all the [`CoordinatorZome`]s for this dna
    pub fn get_all_coordinators(&self) -> Vec<CoordinatorZome> {
        self.coordinator_zomes
            .iter()
            .cloned()
            .map(|(name, def)| CoordinatorZome::new(name, def))
            .collect()
    }

    /// Return a Zome, error if not a WasmZome
    pub fn get_wasm_zome(&self, zome_name: &ZomeName) -> Result<&WasmZome, ZomeError> {
        self.all_zomes()
            .find(|(name, _)| *name == zome_name)
            .map(|(_, def)| def)
            .ok_or_else(|| {
                tracing::error!(
                    "ZomeNotFound: {zome_name}. (get_wasm_zome) Existing zomes: integrity={:?}, coordinator={:?}",
                    self.integrity_zomes,
                    self.coordinator_zomes,
                );
                ZomeError::ZomeNotFound(format!("Wasm zome '{}' not found", &zome_name,))
            })
            .and_then(|def| {
                if let ZomeDef::Wasm(wasm_zome) = def {
                    Ok(wasm_zome)
                } else {
                    Err(ZomeError::NonWasmZome(zome_name.clone()))
                }
            })
    }

    /// Return the Wasm Hash for Zome, error if not a Wasm type Zome
    pub fn get_wasm_zome_hash(&self, zome_name: &ZomeName) -> Result<WasmHash, ZomeError> {
        self.all_zomes()
            .find(|(name, _)| *name == zome_name)
            .map(|(_, def)| def)
            .ok_or_else(|| {
                tracing::error!(
                    "ZomeNotFound: {zome_name}. (get_wasm_zome_hash) Existing zomes: integrity={:?}, coordinator={:?}",
                    self.integrity_zomes,
                    self.coordinator_zomes,
                );
                ZomeError::ZomeNotFound(format!("Hash for wasm zome '{}' not found", &zome_name,))
            })
            .and_then(|def| match def {
                ZomeDef::Wasm(wasm_zome) => Ok(wasm_zome.wasm_hash.clone()),
                _ => Err(ZomeError::NonWasmZome(zome_name.clone())),
            })
    }

    /// Set the DNA's name.
    pub fn set_name(&self, name: String) -> Self {
        let mut clone = self.clone();
        clone.name = name;
        clone
    }

    /// Change the DNA modifiers -- the network seed, properties -- while
    /// leaving the actual DNA code intact.
    pub fn update_modifiers(&self, modifiers: DnaModifiersOpt) -> Self {
        let mut clone = self.clone();
        clone.modifiers = clone.modifiers.update(modifiers);
        clone
    }
}

/// Get a random network seed
#[cfg(feature = "full-dna-def")]
pub fn random_network_seed() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(feature = "full-dna-def")]
impl DnaDefBuilder {
    /// Provide a random network seed
    pub fn random_network_seed(&mut self) -> &mut Self {
        self.modifiers = Some(
            DnaModifiersBuilder::default()
                .network_seed(random_network_seed())
                .build()
                .unwrap(),
        );
        self
    }
}

/// A DnaDef paired with its DnaHash
#[cfg(feature = "full-dna-def")]
pub type DnaDefHashed = HoloHashed<DnaDef>;

#[cfg(feature = "full-dna-def")]
impl HashableContent for DnaDef {
    type HashType = holo_hash::hash_type::Dna;

    fn hash_type(&self) -> Self::HashType {
        holo_hash::hash_type::Dna::new()
    }

    fn hashable_content(&self) -> HashableContentBytes {
        let hash = DnaDefHash {
            modifiers: &self.modifiers,
            integrity_zomes: &self.integrity_zomes,
        };
        HashableContentBytes::Content(
            holochain_serialized_bytes::UnsafeBytes::from(
                holochain_serialized_bytes::encode(&hash)
                    .expect("Could not serialize HashableContent"),
            )
            .into(),
        )
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use holochain_serialized_bytes::prelude::*;

    #[test]
    fn test_update_modifiers() {
        #[derive(Debug, Clone, Serialize, Deserialize, SerializedBytes)]
        struct Props(u32);

        let props = SerializedBytes::try_from(Props(42)).unwrap();

        let mods = DnaModifiers {
            network_seed: "seed".into(),
            properties: ().try_into().unwrap(),
        };

        let opt = DnaModifiersOpt {
            network_seed: None,
            properties: Some(props.clone()),
        };

        let expected = DnaModifiers {
            network_seed: "seed".into(),
            properties: props.clone(),
        };

        assert_eq!(mods.update(opt), expected);
    }
}
