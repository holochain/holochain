//! Type aliases for the various concrete HoloHash types

use crate::{hash_type, HoloHash};
use holochain_serialized_bytes::prelude::*;

// NB: These could be macroized, but if we spell it out, we get better IDE
// support

/// An Agent public signing key. Not really a hash, more of an "identity hash".
pub type AgentPubKey = HoloHash<hash_type::Agent>;

/// The hash of an Entry, if that Entry is not an AgentPubKey
pub type EntryContentHash = HoloHash<hash_type::Content>;

/// The hash of a DnaDef
pub type DnaHash = HoloHash<hash_type::Dna>;

/// The hash of a DhtOp's "unique form" representation
pub type DhtOpHash = HoloHash<hash_type::DhtOp>;

/// The hash of a Header
pub type HeaderHash = HoloHash<hash_type::Header>;

/// The hash of a network ID
pub type NetIdHash = HoloHash<hash_type::NetId>;

/// The hash of some wasm bytecode
pub type WasmHash = HoloHash<hash_type::Wasm>;

/// The hash of an entry.
/// This is a composite of AgentPubKey and EntryContentHash.
pub type EntryHash = HoloHash<hash_type::Entry>;

/// The hash of anything referrable in the DHT.
/// This is a composite of AgentPubKey, EntryContentHash, and HeaderHash
pub type AnyDhtHash = HoloHash<hash_type::AnyDht>;

// TODO: deprecate
// #[deprecated = "alias for HeaderHash"]
#[allow(missing_docs)]
pub type HeaderAddress = HeaderHash;

/// A newtype for a collection of EntryHashes, needed for some wasm return types.
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
