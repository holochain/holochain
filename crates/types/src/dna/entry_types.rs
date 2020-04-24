//! File holding all the structs for handling entry types defined by DNA.

use holochain_serialized_bytes::prelude::*;
use holochain_serialized_bytes::serde::ser::SerializeMap;
use holochain_serialized_bytes::serde::{de::Deserializer, ser::Serializer};
use std::collections::BTreeMap;

/// Enum for Zome EntryType "sharing" property.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Sharing {
    /// Everyone can see.
    Public,

    /// Only local hApp can access.
    Private,

    /// Published, but encrypted.
    Encrypted,
}

impl Sharing {
    /// `true` if the data should be published to the DHT.
    #[rustfmt::skip]
    pub fn can_publish(self) -> bool {
       match self {
           Sharing::Public    => true,
           Sharing::Private   => false,
           Sharing::Encrypted => true,
       }
    }
}

impl Default for Sharing {
    /// Default zome entry_type sharing is "public"
    fn default() -> Self {
        Sharing::Public
    }
}

/// An individual object in a "links_to" array.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Hash)]
pub struct LinksTo {
    /// The target_type of this links_to entry
    #[serde(default)]
    pub target_type: String,

    /// The type of this links_to entry
    #[serde(default)]
    pub link_type: String,
}

impl Default for LinksTo {
    /// Provide defaults for a "links_to" object.
    fn default() -> Self {
        LinksTo {
            target_type: String::new(),
            link_type: String::new(),
        }
    }
}

impl LinksTo {
    /// Allow sane defaults for `LinksTo::new()`.
    pub fn new() -> Self {
        Default::default()
    }
}

/// An a definition of a link from another type (including anchors and system hashes)
/// to the entry type it is part of.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Hash)]
pub struct LinkedFrom {
    /// The target_type of this links_to entry
    #[serde(default)]
    pub base_type: String,

    /// The link_type of this links_to entry
    #[serde(default)]
    pub link_type: String,
}

impl Default for LinkedFrom {
    /// Provide defaults for a "links_to" object.
    fn default() -> Self {
        LinkedFrom {
            base_type: String::new(),
            link_type: String::new(),
        }
    }
}

impl LinkedFrom {
    /// Allow sane defaults for `LinkedFrom::new()`.
    pub fn new() -> Self {
        Default::default()
    }
}

/// Serialize ZomeEntryTypes
pub fn serialize_entry_types<S>(
    entry_types: &ZomeEntryTypes,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut map = serializer.serialize_map(Some(entry_types.len()))?;
    for (k, v) in entry_types {
        map.serialize_entry(&String::from(k.to_owned()), &v)?;
    }
    map.end()
}

/// Deserialize ZomeEntryTypes
pub fn deserialize_entry_types<'de, D>(deserializer: D) -> Result<ZomeEntryTypes, D::Error>
where
    D: Deserializer<'de>,
{
    let serialized_entry_types: BTreeMap<String, EntryTypeDef> =
        BTreeMap::deserialize(deserializer)?;

    Ok(serialized_entry_types
        .into_iter()
        .map(|(k, v)| (EntryType::from(k), v))
        .collect())
}

/// Represents an individual object in the "zome" "entry_types" array.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Hash)]
pub struct EntryTypeDef {
    /// Metdata associated with this entry def (e.g. description, examples, index/UI hints)
    pub properties: SerializedBytes,

    /// The sharing model of this entry type (public, private, encrypted).
    #[serde(default)]
    pub sharing: Sharing,

    /// An array of link definitions associated with this entry type
    #[serde(default)]
    pub links_to: Vec<LinksTo>,

    /// An array of link definitions for links pointing to entries of this type
    #[serde(default)]
    pub linked_from: Vec<LinkedFrom>,
}

impl Default for EntryTypeDef {
    fn default() -> Self {
        EntryTypeDef {
            properties: SerializedBytes::try_from(()).unwrap(),
            sharing: Sharing::default(),
            links_to: Vec::default(),
            linked_from: Vec::default(),
        }
    }
}

impl EntryTypeDef {
    /// Allow sane defaults for `EntryType::new()`.
    pub fn new() -> Self {
        Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_publish() {
        assert!(Sharing::Public.can_publish());
        assert!(!Sharing::Private.can_publish());
    }
}
