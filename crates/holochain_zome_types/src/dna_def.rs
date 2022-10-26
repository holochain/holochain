//! Defines DnaDef struct

use super::zome;
use crate::prelude::*;

#[cfg(feature = "full-dna-def")]
use crate::zome::error::ZomeError;
#[cfg(feature = "full-dna-def")]
use holo_hash::*;

/// Ordered list of integrity zomes in this DNA.
pub type IntegrityZomes = Vec<(ZomeName, zome::IntegrityZomeDef)>;

/// Ordered list of coordinator zomes in this DNA.
pub type CoordinatorZomes = Vec<(ZomeName, zome::CoordinatorZomeDef)>;

/// Placeholder for a real network seed type. See [`DnaDef`].
pub type NetworkSeed = String;

/// Modifiers of this DNA - the network seed, properties and origin time - as
/// opposed to the actual DNA code. These modifiers are included in the DNA
/// hash computation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[cfg_attr(feature = "full-dna-def", derive(derive_builder::Builder))]
pub struct DnaModifiers {
    /// The network seed of a DNA is included in the computation of the DNA hash.
    /// The DNA hash in turn determines the network peers and the DHT, meaning
    /// that only peers with the same DNA hash of a shared DNA participate in the
    /// same network and co-create the DHT. To create a separate DHT for the DNA,
    /// a unique network seed can be specified.
    // TODO: consider Vec<u8> instead (https://github.com/holochain/holochain/pull/86#discussion_r412689085)
    pub network_seed: NetworkSeed,

    /// Any arbitrary application properties can be included in this object.
    #[cfg_attr(feature = "full-dna-def", builder(default = "().try_into().unwrap()"))]
    pub properties: SerializedBytes,

    /// The time used to denote the origin of the network, used to calculate
    /// time windows during gossip.
    /// All Action timestamps must come after this time.
    #[cfg_attr(feature = "full-dna-def", builder(default = "Timestamp::now()"))]
    pub origin_time: Timestamp,
}

impl DnaModifiers {
    /// Replace fields in the modifiers with any Some fields in the argument.
    /// None fields remain unchanged.
    pub fn update(mut self, modifiers: DnaModifiersOpt) -> DnaModifiers {
        self.network_seed = modifiers.network_seed.unwrap_or(self.network_seed);
        self.properties = modifiers.properties.unwrap_or(self.properties);
        self.origin_time = modifiers.origin_time.unwrap_or(self.origin_time);
        self
    }
}

/// [`DnaModifiers`] options of which all are optional.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct DnaModifiersOpt<P = SerializedBytes> {
    /// see [`DnaModifiers`]
    pub network_seed: Option<NetworkSeed>,
    /// see [`DnaModifiers`]
    pub properties: Option<P>,
    /// see [`DnaModifiers`]
    pub origin_time: Option<Timestamp>,
}

impl<P: TryInto<SerializedBytes, Error = E>, E: Into<SerializedBytesError>> Default
    for DnaModifiersOpt<P>
{
    fn default() -> Self {
        Self::none()
    }
}

impl<P: TryInto<SerializedBytes, Error = E>, E: Into<SerializedBytesError>> DnaModifiersOpt<P> {
    /// Constructor with all fields set to `None`
    pub fn none() -> Self {
        Self {
            network_seed: None,
            properties: None,
            origin_time: None,
        }
    }

    /// Serialize the properties field into SerializedBytes
    pub fn serialized(self) -> Result<DnaModifiersOpt<SerializedBytes>, E> {
        let Self {
            network_seed,
            properties,
            origin_time,
        } = self;
        let properties = if let Some(p) = properties {
            Some(p.try_into()?)
        } else {
            None
        };
        Ok(DnaModifiersOpt {
            network_seed,
            properties,
            origin_time,
        })
    }

    /// Return a modified form with the `network_seed` field set
    pub fn with_network_seed(mut self, network_seed: NetworkSeed) -> Self {
        self.network_seed = Some(network_seed);
        self
    }

    /// Return a modified form with the `properties` field set
    pub fn with_properties(mut self, properties: P) -> Self {
        self.properties = Some(properties);
        self
    }

    /// Return a modified form with the `origin_time` field set
    pub fn with_origin_time(mut self, origin_time: Timestamp) -> Self {
        self.origin_time = Some(origin_time);
        self
    }

    /// Check if at least one of the options is set.
    pub fn has_some_option_set(&self) -> bool {
        self.network_seed.is_some() || self.properties.is_some() || self.origin_time.is_some()
    }
}

/// The definition of a DNA: the hash of this data is what produces the DnaHash.
///
/// Historical note: This struct was written before `DnaManifest` appeared.
/// It is included as part of a `DnaFile`. There is still a lot of code that uses
/// this type, but in function, it has mainly been superseded by `DnaManifest`.
/// Hence, this type can basically be thought of as a fully validated, normalized
/// `DnaManifest`
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, SerializedBytes)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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

#[derive(Serialize, Debug, PartialEq, Eq)]
/// A reference to for creating the hash for [`DnaDef`].
struct DnaDefHash<'a> {
    name: &'a String,
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
    pub fn all_zomes(&self) -> impl Iterator<Item = (&ZomeName, &zome::ZomeDef)> {
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
    pub fn get_integrity_zome(
        &self,
        zome_name: &ZomeName,
    ) -> Result<zome::IntegrityZome, ZomeError> {
        self.integrity_zomes
            .iter()
            .find(|(name, _)| name == zome_name)
            .cloned()
            .map(|(name, def)| IntegrityZome::new(name, def))
            .ok_or_else(|| ZomeError::ZomeNotFound(format!("Zome '{}' not found", &zome_name,)))
    }

    /// Check if a zome is an integrity zome.
    pub fn is_integrity_zome(&self, zome_name: &ZomeName) -> bool {
        self.integrity_zomes
            .iter()
            .any(|(name, _)| name == zome_name)
    }

    /// Find a coordinator zome from a [`ZomeName`].
    pub fn get_coordinator_zome(
        &self,
        zome_name: &ZomeName,
    ) -> Result<zome::CoordinatorZome, ZomeError> {
        self.coordinator_zomes
            .iter()
            .find(|(name, _)| name == zome_name)
            .cloned()
            .map(|(name, def)| CoordinatorZome::new(name, def))
            .ok_or_else(|| ZomeError::ZomeNotFound(format!("Zome '{}' not found", &zome_name,)))
    }

    /// Find a any zome from a [`ZomeName`].
    pub fn get_zome(&self, zome_name: &ZomeName) -> Result<zome::Zome, ZomeError> {
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
            .ok_or_else(|| ZomeError::ZomeNotFound(format!("Zome '{}' not found", &zome_name,)))
    }

    /// Get all the [`CoordinatorZome`]s for this dna
    pub fn get_all_coordinators(&self) -> Vec<zome::CoordinatorZome> {
        self.coordinator_zomes
            .iter()
            .cloned()
            .map(|(name, def)| CoordinatorZome::new(name, def))
            .collect()
    }

    /// Return a Zome, error if not a WasmZome
    pub fn get_wasm_zome(&self, zome_name: &ZomeName) -> Result<&zome::WasmZome, ZomeError> {
        self.all_zomes()
            .find(|(name, _)| *name == zome_name)
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

    /// Set the DNA's name.
    pub fn set_name(&self, name: String) -> Self {
        let mut clone = self.clone();
        clone.name = name;
        clone
    }

    /// Change the DNA modifiers -- the network seed, properties and origin time -- while
    /// leaving the actual DNA code intact.
    pub fn update_modifiers(&self, dna_modifiers: DnaModifiersOpt) -> Self {
        let mut clone = self.clone();
        clone.modifiers = clone.modifiers.update(dna_modifiers);
        clone
    }

    /// Get the topology to use for kitsune gossip
    pub fn topology(&self, cutoff: std::time::Duration) -> kitsune_p2p_dht::spacetime::Topology {
        kitsune_p2p_dht::spacetime::Topology::standard(self.modifiers.origin_time, cutoff)
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
        self.modifiers = Some(DnaModifiers {
            network_seed: random_network_seed(),
            properties: SerializedBytes::try_from(()).unwrap(),
            origin_time: Timestamp::now(),
        });
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
            name: &self.name,
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
