//! # DNA Properties Support types

use crate::prelude::*;
use holochain_serialized_bytes::prelude::*;

/// Modifiers of this DNA - the network seed, properties and origin time - as
/// opposed to the actual DNA code. These modifiers are included in the DNA
/// hash computation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
}

impl DnaModifiers {
    /// Replace fields in the modifiers with any Some fields in the argument.
    /// None fields remain unchanged.
    pub fn update(mut self, modifiers: DnaModifiersOpt) -> DnaModifiers {
        self.network_seed = modifiers.network_seed.unwrap_or(self.network_seed);
        self.properties = modifiers.properties.unwrap_or(self.properties);
        self
    }
}

/// [`DnaModifiers`] options of which all are optional.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct DnaModifiersOpt<P = SerializedBytes> {
    /// see [`DnaModifiers`]
    pub network_seed: Option<NetworkSeed>,
    /// see [`DnaModifiers`]
    #[cfg_attr(feature = "schema", schemars(schema_with = "properties_schema"))]
    pub properties: Option<P>,
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
        }
    }

    /// Serialize the properties field into SerializedBytes
    pub fn serialized(self) -> Result<DnaModifiersOpt<SerializedBytes>, E> {
        let Self {
            network_seed,
            properties,
        } = self;
        let properties = if let Some(p) = properties {
            Some(p.try_into()?)
        } else {
            None
        };
        Ok(DnaModifiersOpt {
            network_seed,
            properties,
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

    /// Check if at least one of the options is set.
    pub fn has_some_option_set(&self) -> bool {
        self.network_seed.is_some() || self.properties.is_some()
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

#[cfg(feature = "schema")]
fn properties_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": ["object", "null"],
    })
}
