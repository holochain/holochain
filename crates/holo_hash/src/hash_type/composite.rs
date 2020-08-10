use super::*;

#[cfg(all(test, feature = "serialized-bytes"))]
use holochain_serialized_bytes::prelude::*;

/// The Entry (composite) HashType
#[derive(
    Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize, serde::Serialize,
)]
#[cfg_attr(all(test, feature = "serialized-bytes"), derive(SerializedBytes))]
#[serde(from = "EntrySerial", into = "EntrySerial")]
pub enum Entry {
    /// The hash of an Entry of EntryType::Agent
    Agent,
    /// The hash of any other EntryType
    Content,
}

impl HashType for Entry {
    fn get_prefix(self) -> &'static [u8] {
        match self {
            Entry::Agent => Agent::new().get_prefix(),
            Entry::Content => Content::new().get_prefix(),
        }
    }
    fn hash_name(self) -> &'static str {
        "EntryHash"
    }
}

/// The AnyDht (composite) HashType
#[derive(
    Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize, serde::Serialize,
)]
#[cfg_attr(all(test, feature = "serialized-bytes"), derive(SerializedBytes))]
#[serde(from = "AnyDhtSerial", into = "AnyDhtSerial")]
pub enum AnyDht {
    /// The hash of an Entry
    Entry(Entry),
    /// The hash of a Header
    Header,
}

impl HashType for AnyDht {
    fn get_prefix(self) -> &'static [u8] {
        match self {
            AnyDht::Entry(entry_type) => entry_type.get_prefix(),
            AnyDht::Header => Header::new().get_prefix(),
        }
    }
    fn hash_name(self) -> &'static str {
        "AnyDhtHash"
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
enum EntrySerial {
    /// The hash of an Entry of EntryType::Agent
    Agent(Agent),
    /// The hash of any other EntryType
    Content(Content),
}

impl From<Entry> for EntrySerial {
    fn from(t: Entry) -> Self {
        match t {
            Entry::Agent => EntrySerial::Agent(Agent),
            Entry::Content => EntrySerial::Content(Content),
        }
    }
}

impl From<EntrySerial> for Entry {
    fn from(t: EntrySerial) -> Self {
        match t {
            EntrySerial::Agent(_) => Entry::Agent,
            EntrySerial::Content(_) => Entry::Content,
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
enum AnyDhtSerial {
    /// The hash of an Entry of EntryType::Agent
    Header(Header),
    /// The hash of any other EntryType
    Entry(Entry),
}

impl From<AnyDht> for AnyDhtSerial {
    fn from(t: AnyDht) -> Self {
        match t {
            AnyDht::Header => AnyDhtSerial::Header(Header),
            AnyDht::Entry(e) => AnyDhtSerial::Entry(e),
        }
    }
}

impl From<AnyDhtSerial> for AnyDht {
    fn from(t: AnyDhtSerial) -> Self {
        match t {
            AnyDhtSerial::Header(_) => AnyDht::Header,
            AnyDhtSerial::Entry(e) => AnyDht::Entry(e),
        }
    }
}
