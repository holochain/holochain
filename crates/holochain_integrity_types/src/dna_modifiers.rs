//! # DNA Properties Support types

use std::time::Duration;

use crate::prelude::*;
use holo_hash::DnaHashB64;
use holochain_serialized_bytes::prelude::*;

/// Modifiers of this DNA - the network seed, properties and origin time - as
/// opposed to the actual DNA code. These modifiers are included in the DNA
/// hash computation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
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

    /// The smallest unit of time used for gossip time windows.
    /// You probably don't need to change this.
    #[cfg_attr(feature = "full-dna-def", builder(default = "standard_quantum_time()"))]
    #[cfg_attr(feature = "full-dna-def", serde(default = "standard_quantum_time"))]
    pub quantum_time: Duration,
}

impl DnaModifiers {
    /// Replace fields in the modifiers with any Some fields in the argument.
    /// None fields remain unchanged.
    pub fn update(mut self, modifiers: DnaModifiersOpt) -> DnaModifiers {
        self.network_seed = modifiers.network_seed.unwrap_or(self.network_seed);
        self.properties = modifiers.properties.unwrap_or(self.properties);
        self.origin_time = modifiers.origin_time.unwrap_or(self.origin_time);
        self.quantum_time = modifiers.quantum_time.unwrap_or(self.quantum_time);
        self
    }
}

#[allow(dead_code)]
const fn standard_quantum_time() -> Duration {
    // TODO - put this in a common place that is imported
    //        from both this crate and kitsune_p2p_dht
    //        we do *not* want kitsune_p2p_dht imported into
    //        this crate, because that pulls getrandom into
    //        something that is supposed to be compiled
    //        into integrity wasms.
    Duration::from_secs(60 * 5)
}

/// [`DnaModifiers`] options of which all are optional.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub struct DnaModifiersOpt<P = SerializedBytes> {
    /// see [`DnaModifiers`]
    pub network_seed: Option<NetworkSeed>,
    /// see [`DnaModifiers`]
    pub properties: Option<P>,
    /// see [`DnaModifiers`]
    pub origin_time: Option<Timestamp>,
    /// see [`DnaModifiers`]
    pub quantum_time: Option<Duration>,
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
            quantum_time: None,
        }
    }

    /// Serialize the properties field into SerializedBytes
    pub fn serialized(self) -> Result<DnaModifiersOpt<SerializedBytes>, E> {
        let Self {
            network_seed,
            properties,
            origin_time,
            quantum_time,
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
            quantum_time,
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

    /// Return a modified form with the `quantum_time` field set
    pub fn with_quantum_time(mut self, quantum_time: Duration) -> Self {
        self.quantum_time = Some(quantum_time);
        self
    }

    /// Check if at least one of the options is set.
    pub fn has_some_option_set(&self) -> bool {
        self.network_seed.is_some() || self.properties.is_some() || self.origin_time.is_some()
    }
}

/// Extra parameters that determine the DNA hash.
/// They are set by the conductor at install time and cannot be specified
/// by the DNA developer.
/// They represent different aspects of networking compability.
/// Two conductors using different networking protocols or two different
/// DPKI services will not be able to communicate over the network and are effectively
/// in their own separate networks. By including these parameters in the DNA hash,
/// we make this compatibility explicit, so that two cells will be able to communicate
/// over the same network if and only if their DNA hashes are the same.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub struct DnaRuntime {
    /// A version number which represents network protocol compatibility.
    /// This is set by kitsune and bumped whenever a breaking protocol change is made.
    pub networking_version: u32,

    /// DPKI is fundamental to the workings of agent key management and validation.
    /// Two conductors with different DPKI networks cannot validate each other's agent keys,
    /// effectively cutting them off from each other, so we treat this as a determinant
    /// of network compatibility.
    ///
    /// Note that conductors with no DPKI service installed will be able to talk to conductors
    /// with a DPKI service installed, but not vice versa, so we don't both supporting that
    /// kind of one-way communication.
    pub dpki_hash: Option<DnaHashB64>,
}

#[cfg(feature = "test_utils")]
impl DnaRuntime {
    /// Get a fake value for testing
    pub fn fake() -> Self {
        DnaRuntime {
            networking_version: 42,
            dpki_hash: None,
        }
    }
}

/// Trait to convert from dna properties into specified type
pub trait TryFromDnaProperties {
    /// The error associated with this conversion.
    type Error;

    /// Attempts to deserialize DNA properties into specified type
    fn try_from_dna_properties() -> Result<Self, Self::Error>
    where
        Self: Sized;
}
