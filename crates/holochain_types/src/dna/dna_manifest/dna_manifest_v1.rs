use crate::prelude::*;
use holo_hash::*;
use holochain_zome_types::ZomeName;

/// The structure of data that goes in the DNA bundle manifest,
/// i.e. "dna.yaml"
#[derive(
    Serialize,
    Deserialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    derive_more::Constructor,
    derive_builder::Builder,
)]
#[serde(rename_all = "snake_case")]
pub struct DnaManifestV1 {
    /// The friendly "name" of a Holochain DNA.
    pub name: String,

    /// A UID for uniquifying this Dna.
    // TODO: consider Vec<u8> instead (https://github.com/holochain/holochain/pull/86#discussion_r412689085)
    pub uid: Option<String>,

    /// Any arbitrary application properties can be included in this object.
    pub properties: Option<YamlProperties>,

    /// The time used to denote the origin of the network, used to calculate
    /// time windows during gossip.
    /// All Header timestamps must come after this time.
    pub origin_time: Timestamp,

    /// An array of zomes associated with your DNA.
    /// The order is significant: it determines initialization order.
    pub zomes: Vec<ZomeManifest>,
}

/// Manifest for an individual Zome
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ZomeManifest {
    /// Just a friendly name, no semantic meaning.
    pub name: ZomeName,

    /// The hash of the wasm which defines this zome
    pub hash: Option<WasmHashB64>,

    /// The location of the wasm for this zome
    #[serde(flatten)]
    pub location: ZomeLocation,
}

/// Alias for a suitable representation of zome location
pub type ZomeLocation = mr_bundle::Location;

impl ZomeManifest {
    /// Accessor
    pub fn location(&self) -> &ZomeLocation {
        &self.location
    }
}
