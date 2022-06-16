//! Type aliases for the various concrete HoloHash types

use crate::hash_type;
use crate::HoloHash;

// NB: These could be macroized, but if we spell it out, we get better IDE
// support

// PRIMITIVE HASH TYPES

/// An Agent public signing key. Not really a hash, more of an "identity hash".
pub type AgentPubKey = HoloHash<hash_type::Agent>;

/// The hash of a DnaDef
pub type DnaHash = HoloHash<hash_type::Dna>;

/// The hash of a DhtOp's "unique form" representation
pub type DhtOpHash = HoloHash<hash_type::DhtOp>;

/// The hash of an Entry.
pub type EntryHash = HoloHash<hash_type::Entry>;

/// The hash of a Action
pub type ActionHash = HoloHash<hash_type::Action>;

/// The hash of a network ID
pub type NetIdHash = HoloHash<hash_type::NetId>;

/// The hash of some wasm bytecode
pub type WasmHash = HoloHash<hash_type::Wasm>;

/// The hash of some external data that can't or doesn't exist on the DHT.
pub type ExternalHash = HoloHash<hash_type::External>;

// COMPOSITE HASH TYPES

/// The hash of anything referrable in the DHT.
/// This is a composite of either an EntryHash or a ActionHash
pub type AnyDhtHash = HoloHash<hash_type::AnyDht>;

/// The hash of anything linkable.
pub type AnyLinkableHash = HoloHash<hash_type::AnyLinkable>;

/// The primitive hash types represented by this composite hash
pub enum AnyDhtHashPrimitive {
    /// This is an EntryHash
    Entry(EntryHash),
    /// This is a ActionHash
    Action(ActionHash),
}

/// The primitive hash types represented by this composite hash
pub enum AnyLinkableHashPrimitive {
    /// This is an EntryHash
    Entry(EntryHash),
    /// This is a ActionHash
    Action(ActionHash),
    /// This is an ExternalHash
    External(ExternalHash),
}

impl AnyLinkableHash {
    /// Match on the primitive hash type represented by this composite hash type
    pub fn into_primitive(self) -> AnyLinkableHashPrimitive {
        match self.hash_type() {
            hash_type::AnyLinkable::Entry => {
                AnyLinkableHashPrimitive::Entry(self.retype(hash_type::Entry))
            }
            hash_type::AnyLinkable::Action => {
                AnyLinkableHashPrimitive::Action(self.retype(hash_type::Action))
            }
            hash_type::AnyLinkable::External => {
                AnyLinkableHashPrimitive::External(self.retype(hash_type::External))
            }
        }
    }

    /// If this hash represents an EntryHash, return it, else None
    pub fn into_entry_hash(self) -> Option<EntryHash> {
        if *self.hash_type() == hash_type::AnyLinkable::Entry {
            Some(self.retype(hash_type::Entry))
        } else {
            None
        }
    }

    /// If this hash represents a ActionHash, return it, else None
    pub fn into_action_hash(self) -> Option<ActionHash> {
        if *self.hash_type() == hash_type::AnyLinkable::Action {
            Some(self.retype(hash_type::Action))
        } else {
            None
        }
    }

    /// If this hash represents an ExternalHash, return it, else None
    pub fn into_external_hash(self) -> Option<ExternalHash> {
        if *self.hash_type() == hash_type::AnyLinkable::External {
            Some(self.retype(hash_type::External))
        } else {
            None
        }
    }
}

impl AnyDhtHash {
    /// Match on the primitive hash type represented by this composite hash type
    pub fn into_primitive(self) -> AnyDhtHashPrimitive {
        match self.hash_type() {
            hash_type::AnyDht::Entry => AnyDhtHashPrimitive::Entry(self.retype(hash_type::Entry)),
            hash_type::AnyDht::Action => {
                AnyDhtHashPrimitive::Action(self.retype(hash_type::Action))
            }
        }
    }

    /// If this hash represents an EntryHash, return it, else None
    pub fn into_entry_hash(self) -> Option<EntryHash> {
        if *self.hash_type() == hash_type::AnyDht::Entry {
            Some(self.retype(hash_type::Entry))
        } else {
            None
        }
    }

    /// If this hash represents a ActionHash, return it, else None
    pub fn into_action_hash(self) -> Option<ActionHash> {
        if *self.hash_type() == hash_type::AnyDht::Action {
            Some(self.retype(hash_type::Action))
        } else {
            None
        }
    }
}

impl From<AnyLinkableHash> for AnyDhtHash {
    fn from(hash: AnyLinkableHash) -> Self {
        hash.retype(hash_type::AnyDht::Entry)
    }
}

impl From<ActionHash> for AnyDhtHash {
    fn from(hash: ActionHash) -> Self {
        hash.retype(hash_type::AnyDht::Action)
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

impl From<AnyDhtHash> for ActionHash {
    fn from(hash: AnyDhtHash) -> Self {
        hash.retype(hash_type::Action)
    }
}

impl From<AnyDhtHash> for EntryHash {
    fn from(hash: AnyDhtHash) -> Self {
        hash.retype(hash_type::Entry)
    }
}

impl From<ActionHash> for AnyLinkableHash {
    fn from(hash: ActionHash) -> Self {
        hash.retype(hash_type::AnyLinkable::Action)
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

impl From<AnyLinkableHash> for ActionHash {
    fn from(hash: AnyLinkableHash) -> Self {
        hash.retype(hash_type::Action)
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
