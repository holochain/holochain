use std::path::PathBuf;

use crate::prelude::*;
use holo_hash::*;
// use holochain_zome_types::prelude::*;
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
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct DnaManifestV1 {
    /// The friendly "name" of a Holochain DNA.
    pub name: String,

    /// Specification of integrity zomes and properties.
    ///
    /// Only this affects the [`DnaHash`].
    pub integrity: IntegrityManifest,

    /// Coordinator zomes to install with this DNA.
    ///
    /// Does not affect the [`DnaHash`].
    #[serde(default)]
    pub coordinator: CoordinatorManifest,

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
    ///     DNA in the lineage which contains that DnaHash can be used.
    /// - The app's Coordinator interface is expected to be compatible across the lineage.
    ///     (Though this cannot be enforced, since Coordinators can be swapped out at
    ///      will by the user, the intention is still there.)
    ///
    /// Holochain does nothing to ensure the correctness of the lineage, it is up to
    /// the app developer to make the necessary guarantees.
    #[serde(default)]
    pub lineage: Vec<DnaHashB64>,
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
#[serde(rename_all = "snake_case", deny_unknown_fields)]
/// Manifest for all items that will change the [`DnaHash`].
pub struct IntegrityManifest {
    /// A network seed for uniquifying this DNA. See [`DnaDef`].
    pub network_seed: Option<String>,

    /// Any arbitrary application properties can be included in this object.
    pub properties: Option<YamlProperties>,

    /// The time used to denote the origin of the network, used to calculate
    /// time windows during gossip.
    /// All Action timestamps must come after this time.
    pub origin_time: HumanTimestamp,

    /// An array of zomes associated with your DNA.
    /// The order is significant: it determines initialization order.
    /// The integrity zome manifests.
    pub zomes: Vec<ZomeManifest>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
/// Coordinator zomes.
pub struct CoordinatorManifest {
    /// Coordinator zomes to install with this dna.
    pub zomes: Vec<ZomeManifest>,
}

/// Manifest for an individual Zome
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
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

    /// The location of the wasm dylib for this zome
    /// Useful for iOS.
    #[serde(default)]
    pub dylib: Option<PathBuf>,
}

/// Manifest for integrity zomes that another zome
/// depends on.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
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
