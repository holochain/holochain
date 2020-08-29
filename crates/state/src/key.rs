//! Traits for defining keys and values of databases

use holo_hash::{HashType, HashableContent, HoloHash, HoloHashOf, PrimitiveHashType};
use holochain_serialized_bytes::prelude::*;
use serde::{de::DeserializeOwned, Serialize};
use std::cmp::Ordering;

/// Bytes for "PRE_INT"
/// Prefix for integrated database
const INTEGRATED_PREFIX: u8 = 0x0;
const PREFIX_KEY_SIZE: usize = 46;

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

/// A key for hashes but with a prefix for reusing databases
// #[derive(PartialOrd, Ord, PartialEq, Eq)]
pub struct PrefixKey {
    // TODO: B-02112 Use fixed size array when we have fixed size hash
    //prefix_and_hash: [u8; PREFIX_KEY_SIZE],
    prefix_and_hash: Vec<u8>,
}

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
            Err(o) => panic!("Holochain detected database corruption.\n\nInvalid IntKey: expected {} bytes but got {}", 4, o.len()),
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

impl BufKey for PrefixKey {
    fn from_key_bytes_or_friendly_panic(bytes: &[u8]) -> Self {
        Self::fill_from_raw(bytes)
    }
}

impl AsRef<[u8]> for PrefixKey {
    fn as_ref(&self) -> &[u8] {
        &self.prefix_and_hash[..]
    }
}

impl PrefixKey {
    fn empty() -> Self {
        Self {
            // TODO: B-02112 Use fixed size array when we have fixed size hash
            // prefix_and_hash: [0; PREFIX_KEY_SIZE],
            prefix_and_hash: vec![0; PREFIX_KEY_SIZE],
        }
    }

    // TODO: B-02112 remove this when we have fixed size hash
    // Make then len the same
    fn initialize_key(&mut self, len: usize) {
        self.prefix_and_hash.truncate(len + 1);
    }

    fn fill_from_raw(hash: &[u8]) -> Self {
        // TODO: B-02112 Add this check back
        // if hash.len() != PREFIX_KEY_SIZE {
        //     panic!("Holochain detected database corruption.\n\nInvalid PrefixKey: expected {} bytes but got {}", PREFIX_KEY_SIZE, hash.len());
        // }
        let mut key = Self::empty();

        // TODO: Remove B-02112
        // Already includes the prefix
        key.initialize_key(hash.len() - 1);

        let data_iter = key.prefix_and_hash.iter_mut();
        let hash_iter = hash.iter();
        Self::fill_data(data_iter, hash_iter);
        key
    }

    fn fill(&mut self, prefix: u8, hash: &[u8]) {
        // TODO: Remove B-02112
        self.initialize_key(hash.len());

        self.prefix_and_hash[0] = prefix;
        let data_iter = self.prefix_and_hash.iter_mut().skip(1);
        let hash_iter = hash.iter();
        Self::fill_data(data_iter, hash_iter);
    }

    fn fill_data<'a>(
        data_iter: impl Iterator<Item = &'a mut u8>,
        hash_iter: impl Iterator<Item = &'a u8>,
    ) {
        for (data, hash) in data_iter.zip(hash_iter) {
            *data = *hash;
        }
    }

    /// Create key for integrated databases
    pub fn integrated<C>(hash: &HoloHashOf<C>) -> Self
    where
        C: HashableContent + BufVal + Send + Sync,
        HoloHashOf<C>: BufKey,
        C::HashType: PrimitiveHashType + Send + Sync,
    {
        let mut key = Self::empty();
        key.fill(INTEGRATED_PREFIX, hash.as_ref());
        key
    }

    /// Get the bytes of the hash
    pub fn as_hash_bytes(&self) -> &[u8] {
        &self.prefix_and_hash[1..]
    }
}

impl PartialEq for PrefixKey {
    fn eq(&self, other: &PrefixKey) -> bool {
        self.prefix_and_hash[..] == other.prefix_and_hash[..]
    }
    fn ne(&self, other: &PrefixKey) -> bool {
        self.prefix_and_hash[..] != other.prefix_and_hash[..]
    }
}

impl Eq for PrefixKey {}

impl PartialOrd for PrefixKey {
    fn partial_cmp(&self, other: &PrefixKey) -> Option<Ordering> {
        PartialOrd::partial_cmp(&&self.prefix_and_hash[..], &&other.prefix_and_hash[..])
    }
}

impl Ord for PrefixKey {
    fn cmp(&self, other: &PrefixKey) -> Ordering {
        Ord::cmp(&&self.prefix_and_hash[..], &&other.prefix_and_hash[..])
    }
}
