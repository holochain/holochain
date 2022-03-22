//! Type aliases for the various concrete HoloHash types

use crate::hash_type;
use crate::HoloHash;

// NB: These could be macroized, but if we spell it out, we get better IDE
// support

/// An Agent public signing key. Not really a hash, more of an "identity hash".
pub type AgentPubKey = HoloHash<hash_type::Agent>;

/// The hash of a DnaDef
pub type DnaHash = HoloHash<hash_type::Dna>;

/// The hash of a DhtOp's "unique form" representation
pub type DhtOpHash = HoloHash<hash_type::DhtOp>;

/// The hash of an Entry.
pub type EntryHash = HoloHash<hash_type::Entry>;

/// The hash of a Header
pub type HeaderHash = HoloHash<hash_type::Header>;

/// The hash of a network ID
pub type NetIdHash = HoloHash<hash_type::NetId>;

/// The hash of some wasm bytecode
pub type WasmHash = HoloHash<hash_type::Wasm>;

/// The hash of some external data that can't or doesn't exist on the DHT.
pub type ExternalHash = HoloHash<hash_type::External>;

/// The hash of anything referrable in the DHT.
/// This is a composite of either an EntryHash or a HeaderHash
pub type AnyDhtHash = HoloHash<hash_type::AnyDht>;

/// The hash of anything linkable.
pub type AnyLinkableHash = HoloHash<hash_type::AnyLinkable>;

impl From<HeaderHash> for AnyDhtHash {
    fn from(hash: HeaderHash) -> Self {
        hash.retype(hash_type::AnyDht::Header)
    }
}

impl From<EntryHash> for AnyDhtHash {
    fn from(hash: EntryHash) -> Self {
        hash.retype(hash_type::AnyDht::Entry)
    }
}

// Since an AgentPubKey can be treated as an EntryHash, we can also go straight
// to AnyDhtHash
impl From<AgentPubKey> for AnyDhtHash {
    fn from(hash: AgentPubKey) -> Self {
        hash.retype(hash_type::AnyDht::Entry)
    }
}

impl From<AnyDhtHash> for HeaderHash {
    fn from(hash: AnyDhtHash) -> Self {
        hash.retype(hash_type::Header)
    }
}

impl From<AnyDhtHash> for EntryHash {
    fn from(hash: AnyDhtHash) -> Self {
        hash.retype(hash_type::Entry)
    }
}

impl From<HeaderHash> for AnyLinkableHash {
    fn from(hash: HeaderHash) -> Self {
        hash.retype(hash_type::AnyLinkable::Header)
    }
}

impl From<EntryHash> for AnyLinkableHash {
    fn from(hash: EntryHash) -> Self {
        hash.retype(hash_type::AnyLinkable::Entry)
    }
}

impl From<AgentPubKey> for AnyLinkableHash {
    fn from(hash: AgentPubKey) -> Self {
        hash.retype(hash_type::AnyLinkable::Entry)
    }
}

impl From<ExternalHash> for AnyLinkableHash {
    fn from(hash: ExternalHash) -> Self {
        hash.retype(hash_type::AnyLinkable::External)
    }
}

impl From<AnyLinkableHash> for HeaderHash {
    fn from(hash: AnyLinkableHash) -> Self {
        hash.retype(hash_type::Header)
    }
}

impl From<AnyLinkableHash> for EntryHash {
    fn from(hash: AnyLinkableHash) -> Self {
        hash.retype(hash_type::Entry)
    }
}

impl From<AnyLinkableHash> for ExternalHash {
    fn from(hash: AnyLinkableHash) -> Self {
        hash.retype(hash_type::External)
    }
}

#[cfg(feature = "serialization")]
use holochain_serialized_bytes::prelude::*;

/// A newtype for a collection of EntryHashes, needed for some wasm return types.
#[cfg(feature = "serialization")]
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[repr(transparent)]
#[serde(transparent)]
pub struct EntryHashes(pub Vec<EntryHash>);
