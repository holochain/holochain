//! Defines DnaDef struct

use crate::prelude::*;

#[cfg(feature = "full-dna-def")]
use holochain_integrity_types::DnaModifiersBuilder;

#[cfg(feature = "full-dna-def")]
use crate::zome::ZomeError;
#[cfg(feature = "full-dna-def")]
use holo_hash::*;

#[cfg(feature = "full-dna-def")]
use kitsune_p2p_dht::spacetime::*;

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
#[cfg_attr(feature = "fuzzing", derive(arbitrary::Arbitrary))]
#[cfg_attr(feature = "full-dna-def", derive(derive_builder::Builder))]
#[cfg_attr(feature = "full-dna-def", builder(public))]
pub struct DnaDef {
    /// The friendly "name" of a Holochain DNA.
    #[cfg_attr(
        feature = "full-dna-def",
        builder(default = "\"Generated DnaDef\".to_string()")
    )]
    pub name: String,

    /// Modifiers of this DNA - the network seed, properties and origin time - as
    /// opposed to the actual DNA code. The modifiers are included in the DNA hash
    /// computation.
    pub modifiers: DnaModifiers,

    /// A vector of zomes associated with your DNA.
    pub integrity_zomes: IntegrityZomes,

    /// A vector of zomes that do not affect
    /// the [`DnaHash`].
    pub coordinator_zomes: CoordinatorZomes,
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
    #[tracing::instrument(skip_all)]
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
                    "ZomeNotFound: {zome_name}. Existing zomes: integrity={:?}, coordinator={:?}",
                    self.integrity_zomes,
                    self.coordinator_zomes,
                );
                ZomeError::ZomeNotFound(format!("Zome '{}' not found", &zome_name,))
            })
    }

    /// Check if a zome is an integrity zome.
    #[tracing::instrument(skip_all)]
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
                    "ZomeNotFound: {zome_name}. Existing zomes: integrity={:?}, coordinator={:?}",
                    self.integrity_zomes,
                    self.coordinator_zomes,
                );
                ZomeError::ZomeNotFound(format!("Zome '{}' not found", &zome_name,))
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
                    "ZomeNotFound: {zome_name}. Existing zomes: integrity={:?}, coordinator={:?}",
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
                    "ZomeNotFound: {zome_name}. Existing zomes: integrity={:?}, coordinator={:?}",
                    self.integrity_zomes,
                    self.coordinator_zomes,
                );
                ZomeError::ZomeNotFound(format!("Zome '{}' not found", &zome_name,))
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
                    "ZomeNotFound: {zome_name}. Existing zomes: integrity={:?}, coordinator={:?}",
                    self.integrity_zomes,
                    self.coordinator_zomes,
                );
                ZomeError::ZomeNotFound(format!("Zome '{}' not found", &zome_name,))
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

    /// Change the DNA modifiers -- the network seed, properties and origin time -- while
    /// leaving the actual DNA code intact.
    pub fn update_modifiers(&self, modifiers: DnaModifiersOpt) -> Self {
        let mut clone = self.clone();
        clone.modifiers = clone.modifiers.update(modifiers);
        clone
    }

    /// Get the topology to use for kitsune gossip
    pub fn topology(&self, cutoff: std::time::Duration) -> kitsune_p2p_dht::spacetime::Topology {
        kitsune_p2p_dht::spacetime::Topology {
            space: SpaceDimension::standard(),
            time: TimeDimension::new(self.modifiers.quantum_time),
            time_origin: self.modifiers.origin_time,
            time_cutoff: cutoff,
        }
    }
}

/// Get a random network seed
#[cfg(feature = "full-dna-def")]
pub fn random_network_seed() -> String {
    nanoid::nanoid!()
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
    use kitsune_p2p_dht::spacetime::STANDARD_QUANTUM_TIME;

    #[test]
    fn test_update_modifiers() {
        #[derive(Debug, Clone, Serialize, Deserialize, SerializedBytes)]
        struct Props(u32);

        let props = SerializedBytes::try_from(Props(42)).unwrap();

        let now = Timestamp::now();
        let mods = DnaModifiers {
            network_seed: "seed".into(),
            properties: ().try_into().unwrap(),
            origin_time: Timestamp::HOLOCHAIN_EPOCH,
            quantum_time: STANDARD_QUANTUM_TIME,
        };

        let opt = DnaModifiersOpt {
            network_seed: None,
            properties: Some(props.clone()),
            origin_time: Some(now),
            quantum_time: Some(core::time::Duration::from_secs(60)),
        };

        let expected = DnaModifiers {
            network_seed: "seed".into(),
            properties: props.clone(),
            origin_time: now,
            quantum_time: core::time::Duration::from_secs(60),
        };

        assert_eq!(mods.update(opt), expected);
    }
}
