use crate::prelude::*;
use holo_hash::*;
use mr_bundle::{resource_id_for_path, ResourceIdentifier};
use schemars::JsonSchema;
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
/// manifest_version: "0"
/// name: multi integrity dna
/// integrity:
///   network_seed: 00000000-0000-0000-0000-000000000000
///   properties: ~
///   zomes:
///     - name: zome1
///       path: ../dna1/zomes/zome1.wasm
///     - name: zome2
///       path: ../dna2/zomes/zome1.wasm
/// coordinator:
///   zomes:
///     - name: zome3
///       path: ../dna1/zomes/zome2.wasm
///       dependencies:
///         - name: zome1
///     - name: zome4
///       path: ../dna2/zomes/zome2.wasm
///       dependencies:
///         - name: zome2
/// ```
///
/// When there's only one integrity zome, it will automatically be a dependency
/// of the coordinator zomes. It doesn't need to be specified explicitly.
///
/// Note that while the `dependencies` field is a list, right now there should
/// be **at most one item in this list**.
///
/// ```yaml
/// manifest_version: "0"
/// name: single integrity dna
/// integrity:
///   network_seed: 00000000-0000-0000-0000-000000000000
///   properties: ~
///   zomes:
///     - name: zome1
///       path: ../dna1/zomes/zome1.wasm
/// coordinator:
///   zomes:
///     - name: zome3
///       path: ../dna1/zomes/zome2.wasm
///     - name: zome4
///       path: ../dna2/zomes/zome2.wasm
/// ```
#[serde_as]
#[derive(
    Serialize,
    Deserialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    JsonSchema,
    derive_more::Constructor,
    derive_builder::Builder,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct DnaManifestV0 {
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
    #[builder(default)]
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
    #[cfg(feature = "unstable-migration")]
    #[serde(default)]
    #[builder(default)]
    pub lineage: Vec<DnaHashB64>,
}

impl DnaManifestV0 {
    /// Get all integrity and coordinator zomes.
    pub fn all_zomes(&self) -> impl Iterator<Item = &ZomeManifest> {
        self.integrity
            .zomes
            .iter()
            .chain(self.coordinator.zomes.iter())
    }

    /// Get a mutable iterator over all integrity and coordinator zomes.
    pub fn all_zomes_mut(&mut self) -> impl Iterator<Item = &mut ZomeManifest> {
        self.integrity
            .zomes
            .iter_mut()
            .chain(self.coordinator.zomes.iter_mut())
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
    JsonSchema,
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

    /// An array of zomes associated with your DNA.
    /// The order is significant: it determines initialization order.
    /// The integrity zome manifests.
    pub zomes: Vec<ZomeManifest>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
/// Coordinator zomes.
pub struct CoordinatorManifest {
    /// Coordinator zomes to install with this dna.
    pub zomes: Vec<ZomeManifest>,
}

/// Manifest for an individual Zome
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ZomeManifest {
    /// Just a friendly name, no semantic meaning.
    pub name: ZomeName,

    /// The hash of the wasm which defines this zome
    pub hash: Option<WasmHashB64>,

    /// The location of the WASM for this zome, relative to the manifest.
    pub path: String,

    /// The integrity zomes this zome depends on.
    /// Integrity zomes should have no dependencies; leave this field `null`.
    /// Coordinator zomes may depend on zero or exactly 1 integrity zome.
    /// Currently, a coordinator zome should have **at most one dependency**.
    pub dependencies: Option<Vec<ZomeDependency>>,
}

impl ZomeManifest {
    /// Get the [`ResourceIdentifier`] for this zome.
    pub fn resource_id(&self) -> ResourceIdentifier {
        resource_id_for_path(&self.path).unwrap_or_else(|| format!("{}.wasm", self.name))
    }
}

/// Manifest for integrity zomes that another zome
/// depends on.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ZomeDependency {
    /// The name of the integrity zome this zome depends on.
    pub name: ZomeName,
}
