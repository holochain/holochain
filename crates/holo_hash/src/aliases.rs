//! Type aliases for the various concrete HoloHash types

use crate::hash::ByteArraySerializer;
use crate::hash::HashSerializer;
use crate::hash_type;
use crate::hash_type::*;
use crate::HashType;
use crate::HoloHash;
use crate::PrimitiveHashType;

// NB: These could be macroized, but if we spell it out, we get better IDE
// support

// PRIMITIVE HASH TYPES

/// An Agent public signing key. Not really a hash, more of an "identity hash".
pub type AgentPubKey<H = ByteArraySerializer> = HoloHash<hash_type::Agent, H>;

/// A public key of a pair of signing keys for signing zome calls.
pub type ZomeCallSigningKey = AgentPubKey;

/// The hash of a DnaDef
pub type DnaHash<H = ByteArraySerializer> = HoloHash<hash_type::Dna, H>;

/// The hash of a DhtOp's "unique form" representation
pub type DhtOpHash<H = ByteArraySerializer> = HoloHash<hash_type::DhtOp, H>;

/// The hash of an Entry.
pub type EntryHash<H = ByteArraySerializer> = HoloHash<hash_type::Entry, H>;

/// The hash of an action
pub type ActionHash<H = ByteArraySerializer> = HoloHash<hash_type::Action, H>;

/// The hash of a network ID
pub type NetIdHash<H = ByteArraySerializer> = HoloHash<hash_type::NetId, H>;

/// The hash of some wasm bytecode
pub type WasmHash<H = ByteArraySerializer> = HoloHash<hash_type::Wasm, H>;

/// The hash of some external data that can't or doesn't exist on the DHT.
pub type ExternalHash<H = ByteArraySerializer> = HoloHash<hash_type::External, H>;

// COMPOSITE HASH TYPES

/// The hash of anything referrable in the DHT.
/// This is a composite of either an EntryHash or a ActionHash
pub type AnyDhtHash<H = ByteArraySerializer> = HoloHash<hash_type::AnyDht, H>;

/// The hash of anything linkable.
pub type AnyLinkableHash<H = ByteArraySerializer> = HoloHash<hash_type::AnyLinkable, H>;

/// Alias for AnyLinkableHash. This hash forms the notion of the "basis hash" of an op.
pub type OpBasis = AnyLinkableHash;

/// The primitive hash types represented by this composite hash
pub enum AnyDhtHashPrimitive<S: HashSerializer> {
    /// This is an EntryHash
    Entry(HoloHash<Entry, S>),
    /// This is a ActionHash
    Action(HoloHash<Action, S>),
}

/// The primitive hash types represented by this composite hash
pub enum AnyLinkableHashPrimitive<S: HashSerializer> {
    /// This is an EntryHash
    Entry(HoloHash<Entry, S>),
    /// This is a ActionHash
    Action(HoloHash<Action, S>),
    /// This is an ExternalHash
    External(HoloHash<External, S>),
}

impl<S: HashSerializer> HoloHash<AnyLinkable, S> {
    /// Match on the primitive hash type represented by this composite hash type
    pub fn into_primitive(self) -> AnyLinkableHashPrimitive<S> {
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
    pub fn into_any_dht_hash(self) -> Option<HoloHash<AnyDht, S>> {
        match self.into_primitive() {
            AnyLinkableHashPrimitive::Action(hash) => Some(HoloHash::<AnyDht, S>::from(hash)),
            AnyLinkableHashPrimitive::Entry(hash) => Some(HoloHash::<AnyDht, S>::from(hash)),
            AnyLinkableHashPrimitive::External(_) => None,
        }
    }

    /// If this hash represents an ActionHash, return it, else None
    pub fn into_action_hash(self) -> Option<HoloHash<Action, S>> {
        if *self.hash_type() == hash_type::AnyLinkable::Action {
            Some(self.retype(hash_type::Action))
        } else {
            None
        }
    }

    /// If this hash represents an EntryHash, return it, else None
    pub fn into_entry_hash(self) -> Option<HoloHash<Entry, S>> {
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
    pub fn into_agent_pub_key(self) -> Option<HoloHash<Agent, S>> {
        if *self.hash_type() == hash_type::AnyLinkable::Entry {
            Some(self.retype(hash_type::Agent))
        } else {
            None
        }
    }

    /// If this hash represents an ExternalHash, return it, else None
    pub fn into_external_hash(self) -> Option<HoloHash<External, S>> {
        if *self.hash_type() == hash_type::AnyLinkable::External {
            Some(self.retype(hash_type::External))
        } else {
            None
        }
    }
}

impl<S: HashSerializer> HoloHash<AnyDht, S> {
    /// Match on the primitive hash type represented by this composite hash type
    pub fn into_primitive(self) -> AnyDhtHashPrimitive<S> {
        match self.hash_type() {
            hash_type::AnyDht::Entry => AnyDhtHashPrimitive::Entry(self.retype(hash_type::Entry)),
            hash_type::AnyDht::Action => {
                AnyDhtHashPrimitive::Action(self.retype(hash_type::Action))
            }
        }
    }

    /// If this hash represents an ActionHash, return it, else None
    pub fn into_action_hash(self) -> Option<HoloHash<Action, S>> {
        if *self.hash_type() == hash_type::AnyDht::Action {
            Some(self.retype(hash_type::Action))
        } else {
            None
        }
    }

    /// If this hash represents an EntryHash, return it, else None
    pub fn into_entry_hash(self) -> Option<HoloHash<Entry, S>> {
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
    pub fn into_agent_pub_key(self) -> Option<HoloHash<Agent, S>> {
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

impl<S: HashSerializer> From<HoloHash<AnyDht, S>> for HoloHash<AnyLinkable, S> {
    fn from(hash: HoloHash<AnyDht, S>) -> Self {
        let t = (*hash.hash_type()).into();
        hash.retype(t)
    }
}

impl<S: HashSerializer> TryFrom<HoloHash<AnyLinkable, S>> for HoloHash<AnyDht, S> {
    type Error = CompositeHashConversionError<hash_type::AnyLinkable, S>;

    fn try_from(hash: HoloHash<AnyLinkable, S>) -> Result<Self, Self::Error> {
        hash.clone()
            .into_any_dht_hash()
            .ok_or_else(|| CompositeHashConversionError(hash, "AnyDht".into()))
    }
}

// AnyDhtHash <-> primitives

impl<S: HashSerializer> From<HoloHash<Action, S>> for HoloHash<AnyDht, S> {
    fn from(hash: HoloHash<Action, S>) -> Self {
        hash.retype(hash_type::AnyDht::Action)
    }
}

impl<S: HashSerializer> From<HoloHash<Entry, S>> for HoloHash<AnyDht, S> {
    fn from(hash: HoloHash<Entry, S>) -> Self {
        hash.retype(hash_type::AnyDht::Entry)
    }
}

// Since an AgentPubKey can be treated as an EntryHash, we can also go straight
// to AnyDhtHash
impl<S: HashSerializer> From<HoloHash<Agent, S>> for HoloHash<AnyDht, S> {
    fn from(hash: HoloHash<Agent, S>) -> Self {
        hash.retype(hash_type::AnyDht::Entry)
    }
}

impl<S: HashSerializer> TryFrom<HoloHash<AnyDht, S>> for HoloHash<Action, S> {
    type Error = HashConversionError<hash_type::AnyDht, hash_type::Action, S>;

    fn try_from(hash: HoloHash<AnyDht, S>) -> Result<Self, Self::Error> {
        hash.clone()
            .into_action_hash()
            .ok_or(HashConversionError(hash, hash_type::Action))
    }
}

impl<S: HashSerializer> TryFrom<HoloHash<AnyDht, S>> for HoloHash<Entry, S> {
    type Error = HashConversionError<hash_type::AnyDht, hash_type::Entry, S>;

    fn try_from(hash: HoloHash<AnyDht, S>) -> Result<Self, Self::Error> {
        hash.clone()
            .into_entry_hash()
            .ok_or(HashConversionError(hash, hash_type::Entry))
    }
}

// Since an AgentPubKey can be treated as an EntryHash, we can also go straight
// from AnyDhtHash
impl<S: HashSerializer> TryFrom<HoloHash<AnyDht, S>> for HoloHash<Agent, S> {
    type Error = HashConversionError<hash_type::AnyDht, hash_type::Agent, S>;

    fn try_from(hash: HoloHash<AnyDht, S>) -> Result<Self, Self::Error> {
        hash.clone()
            .into_agent_pub_key()
            .ok_or(HashConversionError(hash, hash_type::Agent))
    }
}

// AnyLinkableHash <-> primitives

impl<S: HashSerializer> From<HoloHash<Action, S>> for HoloHash<AnyLinkable, S> {
    fn from(hash: HoloHash<Action, S>) -> Self {
        hash.retype(hash_type::AnyLinkable::Action)
    }
}

impl<S: HashSerializer> From<HoloHash<Entry, S>> for HoloHash<AnyLinkable, S> {
    fn from(hash: HoloHash<Entry, S>) -> Self {
        hash.retype(hash_type::AnyLinkable::Entry)
    }
}

impl<S: HashSerializer> From<HoloHash<Agent, S>> for HoloHash<AnyLinkable, S> {
    fn from(hash: HoloHash<Agent, S>) -> Self {
        hash.retype(hash_type::AnyLinkable::Entry)
    }
}

impl<S: HashSerializer> From<HoloHash<External, S>> for HoloHash<AnyLinkable, S> {
    fn from(hash: HoloHash<External, S>) -> Self {
        hash.retype(hash_type::AnyLinkable::External)
    }
}

impl<S: HashSerializer> TryFrom<HoloHash<AnyLinkable, S>> for HoloHash<Action, S> {
    type Error = HashConversionError<hash_type::AnyLinkable, hash_type::Action, S>;

    fn try_from(hash: HoloHash<AnyLinkable, S>) -> Result<Self, Self::Error> {
        hash.clone()
            .into_action_hash()
            .ok_or(HashConversionError(hash, hash_type::Action))
    }
}

impl<S: HashSerializer> TryFrom<HoloHash<AnyLinkable, S>> for HoloHash<Entry, S> {
    type Error = HashConversionError<hash_type::AnyLinkable, hash_type::Entry, S>;

    fn try_from(hash: HoloHash<AnyLinkable, S>) -> Result<Self, Self::Error> {
        hash.clone()
            .into_entry_hash()
            .ok_or(HashConversionError(hash, hash_type::Entry))
    }
}

// Since an AgentPubKey can be treated as an EntryHash, we can also go straight
// from AnyLinkableHash
impl<S: HashSerializer> TryFrom<HoloHash<AnyLinkable, S>> for HoloHash<Agent, S> {
    type Error = HashConversionError<hash_type::AnyLinkable, hash_type::Agent, S>;

    fn try_from(hash: HoloHash<AnyLinkable, S>) -> Result<Self, Self::Error> {
        hash.clone()
            .into_agent_pub_key()
            .ok_or(HashConversionError(hash, hash_type::Agent))
    }
}

// Since an AgentPubKey can be treated as an EntryHash, we can also go straight
// from AnyLinkableHash
impl<S: HashSerializer> TryFrom<HoloHash<AnyLinkable, S>> for HoloHash<External, S> {
    type Error = HashConversionError<hash_type::AnyLinkable, hash_type::External, S>;

    fn try_from(hash: HoloHash<AnyLinkable, S>) -> Result<Self, Self::Error> {
        hash.clone()
            .into_external_hash()
            .ok_or(HashConversionError(hash, hash_type::External))
    }
}

#[cfg(feature = "serialization")]
use holochain_serialized_bytes::prelude::*;

// /// A newtype for a collection of EntryHashes, needed for some wasm return types.
// #[cfg(feature = "serialization")]
// #[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
// #[repr(transparent)]
// #[serde(transparent)]
// pub struct EntryHashes<S>(pub Vec<HoloHash<Entry, S>>);

/// Error converting a composite hash into a primitive one, due to type mismatch
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashConversionError<T: HashType, P: PrimitiveHashType, S: HashSerializer>(
    HoloHash<T, S>,
    P,
);

/// Error converting a composite hash into a subset composite hash, due to type mismatch
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositeHashConversionError<T: HashType, S: HashSerializer>(HoloHash<T, S>, String);

#[cfg(feature = "holochain-wasmer")]
use holochain_wasmer_common::WasmErrorInner;

#[cfg(feature = "holochain-wasmer")]
impl<T: HashType, P: PrimitiveHashType, S: HashSerializer> From<HashConversionError<T, P, S>>
    for WasmErrorInner
{
    fn from(err: HashConversionError<T, P, S>) -> Self {
        WasmErrorInner::Guest(format!("{:?}", err))
    }
}

#[cfg(feature = "holochain-wasmer")]
impl<T: HashType, S: HashSerializer> From<CompositeHashConversionError<T, S>> for WasmErrorInner {
    fn from(err: CompositeHashConversionError<T, S>) -> Self {
        WasmErrorInner::Guest(format!("{:?}", err))
    }
}
