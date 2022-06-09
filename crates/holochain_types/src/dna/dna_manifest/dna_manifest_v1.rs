use crate::prelude::*;
use holo_hash::*;
use holochain_zome_types::ZomeName;
use serde_with::serde_as;
use serde_with::FromInto;
use serde_with::PickFirst;

/// The structure of data that goes in the DNA bundle manifest,
/// i.e. "dna.yaml"
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

    /// A UID for uniquifying this Dna.
    // TODO: consider Vec<u8> instead (https://github.com/holochain/holochain/pull/86#discussion_r412689085)
    pub uid: Option<String>,

    /// Any arbitrary application properties can be included in this object.
    pub properties: Option<YamlProperties>,

    /// The time used to denote the origin of the network, used to calculate
    /// time windows during gossip.
    /// All Header timestamps must come after this time.
    #[serde(default = "default_origin_time")]
    pub origin_time: HumanTimestamp,

    /// An array of zomes associated with your DNA.
    /// The order is significant: it determines initialization order.
    #[serde_as(as = "PickFirst<(_, FromInto<Vec<ZomeManifest>>)>")]
    pub zomes: AllZomes,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Integrity and coordinator zomes.
pub struct AllZomes {
    /// The integrity zome manifests.
    pub integrity: Vec<ZomeManifest>,
    /// The coordinator zome manifests.
    #[serde(default)]
    pub coordinator: Vec<ZomeManifest>,
}

impl AllZomes {
    /// Create an empty set of zomes
    pub fn empty() -> Self {
        Self {
            integrity: Default::default(),
            coordinator: Default::default(),
        }
    }

    /// Create an iterator over all zomes
    pub fn iter(&self) -> impl Iterator<Item = &ZomeManifest> {
        self.integrity.iter().chain(self.coordinator.iter())
    }
}

impl From<Vec<ZomeManifest>> for AllZomes {
    fn from(integrity: Vec<ZomeManifest>) -> Self {
        Self {
            integrity,
            coordinator: Default::default(),
        }
    }
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn round_trip_all_zomes() {
        let all_zomes = AllZomes {
            integrity: vec![ZomeManifest {
                name: "1".into(),
                hash: None,
                location: ZomeLocation::Path(PathBuf::from("/test/1.wasm")),
                dependencies: None,
            }],
            coordinator: vec![ZomeManifest {
                name: "2".into(),
                hash: None,
                location: ZomeLocation::Path(PathBuf::from("/test/2.wasm")),
                dependencies: None,
            }],
        };
        let s = serde_yaml::to_string(&all_zomes).unwrap();

        let all_zomes_yaml = r#"
---
integrity:
  - name: "1"
    hash: ~
    path: /test/1.wasm
coordinator:
  - name: "2"
    hash: ~
    path: /test/2.wasm
        "#;

        let r1: AllZomes = serde_yaml::from_str(&all_zomes_yaml).unwrap();
        let r2: AllZomes = serde_yaml::from_str(&s).unwrap();
        assert_eq!(all_zomes, r1);
        assert_eq!(r2, r1);
    }
}
