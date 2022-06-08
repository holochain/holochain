//! Implements base-64 serialization for HoloHashes
//!
//! It's already the case that HoloHash can be deserialized from either a byte
//! array or a base-64 string. This type just specifies how serialization should
//! be done.

use super::*;
use crate::HoloHash;
use crate::{error::HoloHashResult, HashType};

/// A wrapper around HoloHash that `Serialize`s into a base64 string
/// rather than a raw byte array.
#[derive(
    Debug,
    Clone,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Deserialize,
    derive_more::Constructor,
    derive_more::Display,
    derive_more::From,
    derive_more::Into,
    derive_more::AsRef,
)]
#[serde(transparent)]
pub struct HoloHashB64<T: HashType>(HoloHash<T>);

impl<T: HashType> HoloHashB64<T> {
    /// Read a HoloHash from base64 string
    pub fn from_b64_str(str: &str) -> HoloHashResult<Self> {
        let bytes = holo_hash_decode_unchecked(str)?;
        HoloHash::from_raw_39(bytes).map(Into::into)
    }
}

impl<T: HashType> serde::Serialize for HoloHashB64<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&holo_hash_encode(self.0.get_raw_39()))
    }
}

#[cfg(feature = "arbitrary")]
impl<'a, P: PrimitiveHashType> arbitrary::Arbitrary<'a> for HoloHashB64<P> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(HoloHash::arbitrary(u)?.into())
    }
}

// NB: These could be macroized, but if we spell it out, we get better IDE
// support

/// Base64-ready version of AgentPubKey
pub type AgentPubKeyB64 = HoloHashB64<hash_type::Agent>;

/// Base64-ready version of DnaHash
pub type DnaHashB64 = HoloHashB64<hash_type::Dna>;

/// Base64-ready version of DhtOpHash
pub type DhtOpHashB64 = HoloHashB64<hash_type::DhtOp>;

/// Base64-ready version of EntryHash
pub type EntryHashB64 = HoloHashB64<hash_type::Entry>;

/// Base64-ready version of HeaderHash
pub type HeaderHashB64 = HoloHashB64<hash_type::Header>;

/// Base64-ready version of NetIdHash
pub type NetIdHashB64 = HoloHashB64<hash_type::NetId>;

/// Base64-ready version of WasmHash
pub type WasmHashB64 = HoloHashB64<hash_type::Wasm>;

/// Base64-ready version of ExternalHash
pub type ExternalHashB64 = HoloHashB64<hash_type::External>;

/// Base64-ready version of AnyDhtHash
pub type AnyDhtHashB64 = HoloHashB64<hash_type::AnyDht>;

/// Base64-ready version of AnyLinkableHash
pub type AnyLinkableHashB64 = HoloHashB64<hash_type::AnyLinkable>;

impl From<EntryHashB64> for AnyLinkableHash {
    fn from(h: EntryHashB64) -> Self {
        EntryHash::from(h).into()
    }
}

impl From<HeaderHashB64> for AnyLinkableHash {
    fn from(h: HeaderHashB64) -> Self {
        HeaderHash::from(h).into()
    }
}

impl From<EntryHashB64> for AnyDhtHash {
    fn from(h: EntryHashB64) -> Self {
        EntryHash::from(h).into()
    }
}

impl From<HeaderHashB64> for AnyDhtHash {
    fn from(h: HeaderHashB64) -> Self {
        HeaderHash::from(h).into()
    }
}
