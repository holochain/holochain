use crate::prelude::*;
use holo_hash::*;
use holochain_zome_types::ZomeName;
use serde_with::serde_as;

/// The structure of data that goes in the DNA bundle manifest "dna.yaml".
///
/// Navigating through this structure reveals all configurable DNA properties.
///
/// # Examples
///
/// An example "dna.yaml" with 2 integrity and 2 coordinator zomes:
///
/// ```yaml
/// manifest_version: "1"
/// name: multi integrity dna
/// integrity:
///   network_seed: 00000000-0000-0000-0000-000000000000
///   properties: ~
///   origin_time: 2022-02-11T23:05:19.470323Z
///   zomes:
///     - name: zome1
///       bundled: ../dna1/zomes/zome1.wasm
///     - name: zome2
///       bundled: ../dna2/zomes/zome1.wasm
/// coordinator:
///   zomes:
///     - name: zome3
///       bundled: ../dna1/zomes/zome2.wasm
///       dependencies:
///         - name: zome1
///     - name: zome4
///       bundled: ../dna2/zomes/zome2.wasm
///       dependencies:
///         - name: zome1
///         - name: zome2
/// ```
///
/// When there's only one integrity zome, it will automatically be a dependency
/// of the coordinator zomes. It doesn't need to be specified explicitly.
///
/// ```yaml
/// manifest_version: "1"
/// name: single integrity dna
/// integrity:
///   network_seed: 00000000-0000-0000-0000-000000000000
///   properties: ~
///   origin_time: 2022-02-11T23:05:19.470323Z
///   zomes:
///     - name: zome1
///       bundled: ../dna1/zomes/zome1.wasm
/// coordinator:
///   zomes:
///     - name: zome3
///       bundled: ../dna1/zomes/zome2.wasm
///     - name: zome4
///       bundled: ../dna2/zomes/zome2.wasm
/// ```

#[serde_as]
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

    /// Specification of integrity zomes and properties.
    ///
    /// Only this affects the [`DnaHash`].
    pub integrity: IntegrityManifest,

    #[serde(default)]
    /// Coordinator zomes to install with this DNA.
    ///
    /// Does not affect the [`DnaHash`].
    pub coordinator: CoordinatorManifest,
}

impl DnaManifestV1 {
    /// Get all integrity and coordinator zomes.
    pub fn all_zomes(&self) -> impl Iterator<Item = &ZomeManifest> {
        self.integrity
            .zomes
            .iter()
            .chain(self.coordinator.zomes.iter())
    }
}

#[serde_as]
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
/// Manifest for all items that will change the [`DnaHash`].
pub struct IntegrityManifest {
    /// A network seed for uniquifying this DNA.
    // TODO: consider Vec<u8> instead (https://github.com/holochain/holochain/pull/86#discussion_r412689085)
    pub network_seed: Option<String>,

    /// Any arbitrary application properties can be included in this object.
    pub properties: Option<YamlProperties>,

    /// The time used to denote the origin of the network, used to calculate
    /// time windows during gossip.
    /// All Action timestamps must come after this time.
    #[serde(default = "default_origin_time")]
    pub origin_time: HumanTimestamp,

    /// An array of zomes associated with your DNA.
    /// The order is significant: it determines initialization order.
    /// The integrity zome manifests.
    pub zomes: Vec<ZomeManifest>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
/// Coordinator zomes.
pub struct CoordinatorManifest {
    /// Coordinator zomes to install with this dna.
    pub zomes: Vec<ZomeManifest>,
}

fn default_origin_time() -> HumanTimestamp {
    // Jan 1, 2022, 12:00:00 AM UTC
    Timestamp::HOLOCHAIN_EPOCH.into()
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

    /// The integrity zomes this zome depends on.
    /// The order of these must match the order the types
    /// are used in the zome.
    pub dependencies: Option<Vec<ZomeDependency>>,
}

/// Manifest for integrity zomes that another zome
/// depends on.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ZomeDependency {
    /// The name of the integrity zome this zome depends on.
    pub name: ZomeName,
}

/// Alias for a suitable representation of zome location
pub type ZomeLocation = mr_bundle::Location;

impl ZomeManifest {
    /// Accessor
    pub fn location(&self) -> &ZomeLocation {
        &self.location
    }
}
