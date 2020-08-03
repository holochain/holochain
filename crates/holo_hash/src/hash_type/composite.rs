use super::*;

/// The Entry (composite) HashType
#[derive(
    Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize, serde::Serialize,
)]
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
