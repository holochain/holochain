//! Implements base-64 serialization for HoloHashes
//!
//! It's already the case that HoloHash can be deserialized from either a byte
//! array or a base-64 string. This type just specifies how serialization should
//! be done.

use super::*;
use crate::hash::Base64Serializer;
use crate::HoloHash;
use crate::{error::HoloHashResult, HashType};

/// A wrapper around HoloHash that `Serialize`s into a base64 string
/// rather than a raw byte array.
pub type HoloHashB64<T> = HoloHash<T, Base64Serializer>;

impl<T: HashType> HoloHashB64<T> {
    /// Read a HoloHash from base64 string
    pub fn from_b64_str(str: &str) -> HoloHashResult<Self> {
        let bytes = holo_hash_decode_unchecked(str)?;
        HoloHash::from_raw_39(bytes).map(|h| h.change_serialization())
    }
}

macro_rules! impl_froms {
    ($t: ty) => {
        impl From<HoloHash<$t, $crate::hash::ByteArraySerializer>>
            for HoloHash<$t, $crate::hash::Base64Serializer>
        {
            fn from(h: HoloHash<$t, $crate::hash::ByteArraySerializer>) -> Self {
                h.change_serialization()
            }
        }

        impl From<HoloHash<$t, $crate::hash::Base64Serializer>>
            for HoloHash<$t, $crate::hash::ByteArraySerializer>
        {
            fn from(h: HoloHash<$t, $crate::hash::Base64Serializer>) -> Self {
                h.change_serialization()
            }
        }
    };
}

// NB: These could be macroized, but if we spell it out, we get better IDE
// support

/// Base64-ready version of AgentPubKey
pub type AgentPubKeyB64 = HoloHashB64<hash_type::Agent>;
impl_froms!(hash_type::Agent);

/// Base64-ready version of DnaHash
pub type DnaHashB64 = HoloHashB64<hash_type::Dna>;
impl_froms!(hash_type::Dna);

/// Base64-ready version of DhtOpHash
pub type DhtOpHashB64 = HoloHashB64<hash_type::DhtOp>;
impl_froms!(hash_type::DhtOp);

/// Base64-ready version of EntryHash
pub type EntryHashB64 = HoloHashB64<hash_type::Entry>;
impl_froms!(hash_type::Entry);

/// Base64-ready version of ActionHash
pub type ActionHashB64 = HoloHashB64<hash_type::Action>;
impl_froms!(hash_type::Action);

/// Base64-ready version of NetIdHash
pub type NetIdHashB64 = HoloHashB64<hash_type::NetId>;
impl_froms!(hash_type::NetId);

/// Base64-ready version of WasmHash
pub type WasmHashB64 = HoloHashB64<hash_type::Wasm>;
impl_froms!(hash_type::Wasm);

/// Base64-ready version of ExternalHash
pub type ExternalHashB64 = HoloHashB64<hash_type::External>;
impl_froms!(hash_type::External);

/// Base64-ready version of AnyDhtHash
pub type AnyDhtHashB64 = HoloHashB64<hash_type::AnyDht>;
impl_froms!(hash_type::AnyDht);

/// Base64-ready version of AnyLinkableHash
pub type AnyLinkableHashB64 = HoloHashB64<hash_type::AnyLinkable>;
impl_froms!(hash_type::AnyLinkable);
