//! Holochain's header variations
//!
//! All header variations contain the fields `author` and `timestamp`.
//! Furthermore, all variations besides pub struct `Dna` (which is the first header
//! in a chain) contain the field `prev_header`.

#![allow(missing_docs)]

use crate::entry::EntryAddress;

pub type ZomeId = u8;

use crate::{prelude::*, time::Iso8601};

/// defines a timestamp as used in a header
pub type Timestamp = Iso8601;

/// header for a DNA entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct Dna {
    pub author: AgentHash,
    pub timestamp: Timestamp,
    // No previous header, because DNA is always first chain entry
    pub hash: DnaHash,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct LinkAdd {
    pub author: AgentHash,
    pub timestamp: Timestamp,
    pub prev_header: HeaderHash,

    pub base: HoloHash,   // Not HoloHash, but HeaderHash or EntryHash or AgentHash
    pub target: HoloHash, // Not HoloHash, but HeaderHash or EntryHash or AgentHash
    pub tag: SerializedBytes,
    pub link_type: SerializedBytes,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct LinkRemove {
    pub author: AgentHash,
    pub timestamp: Timestamp,
    pub prev_header: HeaderHash,
    /// The address of the `LinkAdd` being reversed
    pub link_add_hash: HoloHash, // not HoloHash byt LinkAddHash or maybe its HeaderHash?
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct ChainOpen {
    pub author: AgentHash,
    pub timestamp: Timestamp,
    pub prev_header: HeaderHash,

    pub prev_dna_hash: DnaHash,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct ChainClose {
    pub author: AgentHash,
    pub timestamp: Timestamp,
    pub prev_header: HeaderHash,

    pub new_dna_hash: DnaHash,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct EntryCreate {
    pub author: AgentHash,
    pub timestamp: Timestamp,
    pub prev_header: HeaderHash,

    pub entry_type: EntryType,
    pub entry_address: EntryAddress,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct EntryUpdate {
    pub author: AgentHash,
    pub timestamp: Timestamp,
    pub prev_header: HeaderHash,

    pub replaces: HoloHash, // not HoloHash but EntryHash or HeaderHash ??

    pub entry_type: EntryType,
    pub entry_address: EntryAddress,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct EntryDelete {
    pub author: AgentHash,
    pub timestamp: Timestamp,
    pub prev_header: HeaderHash,

    /// Hash Address of the Element being deleted
    pub removes: HoloHash, // not HoloHash but EntryHash or HeaderHash ??
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub enum EntryType {
    AgentKey,
    // Stores the App's provided filtration data
    // FIXME: Change this if we are keeping Zomes
    App(AppEntryType),
    CapTokenClaim,
    CapTokenGrant,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct AppEntryType {
    id: Vec<u8>,
    zome_id: ZomeId,
    is_public: bool,
}
