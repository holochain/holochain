//! Type aliases for the various concrete HoloHash types

use crate::hash_type;
use crate::HashType;
use crate::HoloHash;
use crate::PrimitiveHashType;

// NB: These could be macroized, but if we spell it out, we get better IDE
// support

// PRIMITIVE HASH TYPES

/// An Agent public signing key. Not really a hash, more of an "identity hash".
pub type AgentPubKey = HoloHash<hash_type::Agent>;

/// A public key of a pair of signing keys for signing zome calls.
pub type ZomeCallSigningKey = AgentPubKey;

/// The hash of a DnaDef
pub type DnaHash = HoloHash<hash_type::Dna>;

/// The hash of a DhtOp's "unique form" representation
pub type DhtOpHash = HoloHash<hash_type::DhtOp>;

/// The hash of an Entry.
pub type EntryHash = HoloHash<hash_type::Entry>;

/// The hash of an action
pub type ActionHash = HoloHash<hash_type::Action>;

/// The hash of a network ID
pub type NetIdHash = HoloHash<hash_type::NetId>;

/// The hash of some wasm bytecode
pub type WasmHash = HoloHash<hash_type::Wasm>;

/// The hash of a Warrant
pub type WarrantHash = HoloHash<hash_type::Warrant>;

/// The hash of some external data that can't or doesn't exist on the DHT.
pub type ExternalHash = HoloHash<hash_type::External>;

// COMPOSITE HASH TYPES

/// The hash of anything referrable in the DHT.
/// This is a composite of either an EntryHash or a ActionHash
pub type AnyDhtHash = HoloHash<hash_type::AnyDht>;

/// The hash of anything linkable.
pub type AnyLinkableHash = HoloHash<hash_type::AnyLinkable>;

/// Alias for AnyLinkableHash. This hash forms the notion of the "basis hash" of an op.
pub type OpBasis = AnyLinkableHash;

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

    /// Downcast to AnyDhtHash if this is not an external hash
    pub fn into_any_dht_hash(self) -> Option<AnyDhtHash> {
        match self.into_primitive() {
            AnyLinkableHashPrimitive::Action(hash) => Some(AnyDhtHash::from(hash)),
            AnyLinkableHashPrimitive::Entry(hash) => Some(AnyDhtHash::from(hash)),
            AnyLinkableHashPrimitive::External(_) => None,
        }
    }

    /// If this hash represents an ActionHash, return it, else None
    pub fn into_action_hash(self) -> Option<ActionHash> {
        if *self.hash_type() == hash_type::AnyLinkable::Action {
            Some(self.retype(hash_type::Action))
        } else {
            None
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

    /// If this hash represents an EntryHash which is actually an AgentPubKey,
    /// return it, else None.
    //
    // NOTE: this is not completely correct since EntryHash should be a composite type,
    //       with a fallible conversion to Agent
    pub fn into_agent_pub_key(self) -> Option<AgentPubKey> {
        if *self.hash_type() == hash_type::AnyLinkable::Entry {
            Some(self.retype(hash_type::Agent))
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

    /// If this hash represents an ActionHash, return it, else None
    pub fn into_action_hash(self) -> Option<ActionHash> {
        if *self.hash_type() == hash_type::AnyDht::Action {
            Some(self.retype(hash_type::Action))
        } else {
            None
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

    /// If this hash represents an EntryHash which is actually an AgentPubKey,
    /// return it, else None.
    //
    // NOTE: this is not completely correct since EntryHash should be a composite type,
    //       with a fallible conversion to Agent
    pub fn into_agent_pub_key(self) -> Option<AgentPubKey> {
        if *self.hash_type() == hash_type::AnyDht::Entry {
            Some(self.retype(hash_type::Agent))
        } else {
            None
        }
    }
}

// We have From impls for:
// - any primitive hash into a composite hash which contains that primitive
// - any composite hash which is a subset of another composite hash (AnyDht < AnyLinkable)
// - converting between EntryHash and AgentPubKey
// All other conversions, viz. the inverses of the above, are TryFrom conversions, since to
// go from a superset to a subset is only valid in certain cases.
//
// TODO: DRY up with macros

// AnyDhtHash <-> AnyLinkableHash

impl From<AnyDhtHash> for AnyLinkableHash {
    fn from(hash: AnyDhtHash) -> Self {
        let t = (*hash.hash_type()).into();
        hash.retype(t)
    }
}

impl TryFrom<AnyLinkableHash> for AnyDhtHash {
    type Error = CompositeHashConversionError<hash_type::AnyLinkable>;

    fn try_from(hash: AnyLinkableHash) -> Result<Self, Self::Error> {
        hash.clone()
            .into_any_dht_hash()
            .ok_or_else(|| CompositeHashConversionError(hash, "AnyDht".into()))
    }
}

// AnyDhtHash <-> primitives

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

impl TryFrom<AnyDhtHash> for ActionHash {
    type Error = HashConversionError<hash_type::AnyDht, hash_type::Action>;

    fn try_from(hash: AnyDhtHash) -> Result<Self, Self::Error> {
        hash.clone()
            .into_action_hash()
            .ok_or(HashConversionError(hash, hash_type::Action))
    }
}

impl TryFrom<AnyDhtHash> for EntryHash {
    type Error = HashConversionError<hash_type::AnyDht, hash_type::Entry>;

    fn try_from(hash: AnyDhtHash) -> Result<Self, Self::Error> {
        hash.clone()
            .into_entry_hash()
            .ok_or(HashConversionError(hash, hash_type::Entry))
    }
}

// Since an AgentPubKey can be treated as an EntryHash, we can also go straight
// from AnyDhtHash
impl TryFrom<AnyDhtHash> for AgentPubKey {
    type Error = HashConversionError<hash_type::AnyDht, hash_type::Agent>;

    fn try_from(hash: AnyDhtHash) -> Result<Self, Self::Error> {
        hash.clone()
            .into_agent_pub_key()
            .ok_or(HashConversionError(hash, hash_type::Agent))
    }
}

// AnyLinkableHash <-> primitives

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

impl TryFrom<AnyLinkableHash> for ActionHash {
    type Error = HashConversionError<hash_type::AnyLinkable, hash_type::Action>;

    fn try_from(hash: AnyLinkableHash) -> Result<Self, Self::Error> {
        hash.clone()
            .into_action_hash()
            .ok_or(HashConversionError(hash, hash_type::Action))
    }
}

impl TryFrom<AnyLinkableHash> for EntryHash {
    type Error = HashConversionError<hash_type::AnyLinkable, hash_type::Entry>;

    fn try_from(hash: AnyLinkableHash) -> Result<Self, Self::Error> {
        hash.clone()
            .into_entry_hash()
            .ok_or(HashConversionError(hash, hash_type::Entry))
    }
}

// Since an AgentPubKey can be treated as an EntryHash, we can also go straight
// from AnyLinkableHash
impl TryFrom<AnyLinkableHash> for AgentPubKey {
    type Error = HashConversionError<hash_type::AnyLinkable, hash_type::Agent>;

    fn try_from(hash: AnyLinkableHash) -> Result<Self, Self::Error> {
        hash.clone()
            .into_agent_pub_key()
            .ok_or(HashConversionError(hash, hash_type::Agent))
    }
}

// Since an AgentPubKey can be treated as an EntryHash, we can also go straight
// from AnyLinkableHash
impl TryFrom<AnyLinkableHash> for ExternalHash {
    type Error = HashConversionError<hash_type::AnyLinkable, hash_type::External>;

    fn try_from(hash: AnyLinkableHash) -> Result<Self, Self::Error> {
        hash.clone()
            .into_external_hash()
            .ok_or(HashConversionError(hash, hash_type::External))
    }
}

#[cfg(feature = "serialization")]
use holochain_serialized_bytes::prelude::*;

/// A newtype for a collection of EntryHashes, needed for some wasm return types.
#[cfg(feature = "serialization")]
#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[repr(transparent)]
#[serde(transparent)]
pub struct EntryHashes(pub Vec<EntryHash>);

/// Error converting a composite hash into a primitive one, due to type mismatch
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashConversionError<T: HashType, P: PrimitiveHashType>(HoloHash<T>, P);

/// Error converting a composite hash into a subset composite hash, due to type mismatch
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositeHashConversionError<T: HashType>(HoloHash<T>, String);

#[cfg(feature = "holochain-wasmer")]
use holochain_wasmer_common::WasmErrorInner;

#[cfg(feature = "holochain-wasmer")]
impl<T: HashType, P: PrimitiveHashType> From<HashConversionError<T, P>> for WasmErrorInner {
    fn from(err: HashConversionError<T, P>) -> Self {
        WasmErrorInner::Guest(format!("{:?}", err))
    }
}

#[cfg(feature = "holochain-wasmer")]
impl<T: HashType> From<CompositeHashConversionError<T>> for WasmErrorInner {
    fn from(err: CompositeHashConversionError<T>) -> Self {
        WasmErrorInner::Guest(format!("{:?}", err))
    }
}
