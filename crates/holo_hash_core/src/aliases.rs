use crate::{hash_type, HoloHashImpl};
use holochain_serialized_bytes::prelude::*;

// NB: These could be macroized, but if we spell it out, we get better IDE
// support
pub type AgentPubKey = HoloHashImpl<hash_type::Agent>;
pub type EntryContentHash = HoloHashImpl<hash_type::Content>;
pub type DnaHash = HoloHashImpl<hash_type::Dna>;
pub type DhtOpHash = HoloHashImpl<hash_type::DhtOp>;
pub type HeaderHash = HoloHashImpl<hash_type::Header>;
pub type NetIdHash = HoloHashImpl<hash_type::NetId>;
pub type WasmHash = HoloHashImpl<hash_type::Wasm>;

pub type EntryHash = HoloHashImpl<hash_type::Entry>;
pub type AnyDhtHash = HoloHashImpl<hash_type::AnyDht>;

// TODO: deprecate
// #[deprecated = "alias for HeaderHash"]
pub type HeaderAddress = HeaderHash;

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[repr(transparent)]
#[serde(transparent)]
pub struct EntryHashes(pub Vec<EntryHash>);

impl From<AgentPubKey> for EntryHash {
    fn from(hash: AgentPubKey) -> Self {
        hash.retype(hash_type::Entry::Agent)
    }
}

impl From<EntryContentHash> for EntryHash {
    fn from(hash: EntryContentHash) -> Self {
        hash.retype(hash_type::Entry::Content)
    }
}

impl From<HeaderHash> for AnyDhtHash {
    fn from(hash: HeaderHash) -> Self {
        hash.retype(hash_type::AnyDht::Header)
    }
}

impl From<EntryHash> for AnyDhtHash {
    fn from(hash: EntryHash) -> Self {
        let hash_type = *hash.hash_type();
        hash.retype(hash_type::AnyDht::Entry(hash_type))
    }
}

impl From<AgentPubKey> for AnyDhtHash {
    fn from(hash: AgentPubKey) -> Self {
        hash.retype(hash_type::AnyDht::Entry(hash_type::Entry::Agent))
    }
}
