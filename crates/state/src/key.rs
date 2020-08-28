//! Traits for defining keys and values of databases

use holo_hash::{HashType, HoloHash};
use holochain_serialized_bytes::prelude::*;
use serde::{de::DeserializeOwned, Serialize};

/// Any key type used in a [KvStore] or [KvvStore] must implement this trait
pub trait BufKey: Sized + Ord + Eq + AsRef<[u8]> + Send + Sync {
    /// Convert to the key bytes.
    ///
    /// This is provided by the AsRef impl by default, but can be overridden if
    /// there is a way to go into a Vec without an allocation
    fn to_key_bytes(self) -> Vec<u8> {
        self.as_ref().to_vec()
    }

    /// The inverse of to_key_bytes. **This can panic!**.
    /// Only call this on bytes which were created by `to_key_bytes`.
    /// The method is named as such to remind implementors that any potential
    /// panic should be a friendly message that suggests that the database may
    /// have been corrupted
    fn from_key_bytes_or_friendly_panic(bytes: &[u8]) -> Self;
}

/// Trait alias for the combination of constraints needed for keys in [KvIntStore](kv_int::KvIntStore)
pub trait BufIntKey: Ord + Eq + rkv::store::integer::PrimitiveInt + Send + Sync {}
impl<T> BufIntKey for T where T: Ord + Eq + rkv::store::integer::PrimitiveInt + Send + Sync {}

/// Trait alias for the combination of constraints needed for values in [KvStore](kv::KvStore) and [KvIntStore](kv_int::KvIntStore)
pub trait BufVal: Clone + Serialize + DeserializeOwned + std::fmt::Debug + Send + Sync {}
impl<T> BufVal for T where T: Clone + Serialize + DeserializeOwned + std::fmt::Debug + Send + Sync {}

/// Trait alias for the combination of constraints needed for values in [KvvStore]
pub trait BufMultiVal: Ord + Eq + Clone + Serialize + DeserializeOwned + Send + Sync {}
impl<T> BufMultiVal for T where T: Ord + Eq + Clone + Serialize + DeserializeOwned + Send + Sync {}

/// Used for keys into integer-keyed LMDB stores.
///
/// This strange type is constrained by both rkv's interface, and our own
/// database abstractions
#[derive(Copy, PartialOrd, Ord, PartialEq, Eq, Clone, Serialize, serde::Deserialize)]
pub struct IntKey([u8; 4]);

impl rkv::store::integer::PrimitiveInt for IntKey {}

impl BufKey for IntKey {
    fn from_key_bytes_or_friendly_panic(vec: &[u8]) -> Self {
        let boxed_slice = vec.to_vec().into_boxed_slice();
        let boxed_array: Box<[u8; 4]> = match boxed_slice.try_into() {
            Ok(ba) => ba,
            Err(o) => panic!("Expected a Vec of length {} but it was {}", 4, o.len()),
        };
        IntKey(*boxed_array)
    }
}

impl AsRef<[u8]> for IntKey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<u32> for IntKey {
    fn from(u: u32) -> Self {
        use byteorder::{BigEndian, WriteBytesExt};
        let mut wtr = vec![];
        wtr.write_u32::<BigEndian>(u).unwrap();
        Self::from_key_bytes_or_friendly_panic(&wtr)
    }
}

impl From<IntKey> for u32 {
    fn from(k: IntKey) -> u32 {
        use byteorder::{BigEndian, ByteOrder};
        BigEndian::read_u32(&k.0)
    }
}

impl<T: HashType + Send + Sync> BufKey for HoloHash<T> {
    fn from_key_bytes_or_friendly_panic(bytes: &[u8]) -> Self {
        // FIXME: change after [ B-02112 ]
        tracing::error!("This is NOT correct for AnyDhtHash!");
        Self::from_raw_bytes_and_type(bytes.to_vec(), T::default())
    }
}

/// Use this as the key type for LMDB databases which should only have one key.
///
/// This type can only be used as one possible reference
#[derive(derive_more::Display, PartialOrd, Ord, PartialEq, Eq)]
pub struct UnitDbKey;

impl AsRef<[u8]> for UnitDbKey {
    fn as_ref(&self) -> &[u8] {
        ARBITRARY_BYTE_SLICE
    }
}

impl BufKey for UnitDbKey {
    fn to_key_bytes(self) -> Vec<u8> {
        ARBITRARY_BYTE_SLICE.to_vec()
    }

    fn from_key_bytes_or_friendly_panic(bytes: &[u8]) -> Self {
        assert_eq!(bytes, ARBITRARY_BYTE_SLICE);
        Self
    }
}

impl From<()> for UnitDbKey {
    fn from(_: ()) -> Self {
        Self
    }
}

static ARBITRARY_BYTE_SLICE: &[u8] = &[0];
