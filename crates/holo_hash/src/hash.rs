//! Defines the HoloHash type, used for all hashes in Holochain.
//!
//! HoloHashes come in a variety of types. See the `hash_type::primitive`
//! module for the full list.
//!
//! HoloHashes are serialized as a plain 39-byte sequence.
//! The structure is like so:
//!
//! ```text
//! PPPCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCLLLL
//! ^  ^                                  ^
//!  \  \---------"untyped"--------------/
//!   \                                 /
//!    \-------------"full"------------/
//!
//! P: 3 byte prefix to indicate hash type
//! C: 32 byte hash, the "core"
//! L: 4 byte hash of the core hash, for DHT location
//! ```
//!
//! The 36 bytes which exclude the initial 3-byte type prefix are known
//! throughout the codebase as the "untyped" hash
//!
//! The complete 39 bytes together are known as the "full" hash

use kitsune_p2p_dht_arc::DhtLocation;

use crate::error::HoloHashResult;
use crate::has_hash::HasHash;
use crate::HashType;
use crate::PrimitiveHashType;

#[cfg(feature = "hashing")]
use crate::encode;

/// Ways of serializing a HoloHash
pub trait HashSerializer:
    Clone
    + std::fmt::Debug
    + Eq
    + Ord
    + std::hash::Hash
    + serde::Serialize
    + serde::de::DeserializeOwned
{
    /// Produce the value corresponding to this type
    fn get() -> HashSerialization;
}

/// Ways of serializing a HoloHash
pub enum HashSerialization {
    /// This hash is serialized as a byte array
    ByteArray,
    /// This hash is serialized as a base64 string
    Base64,
    /// This hash is serialized as a byte sequence (rather than a byte array).
    ByteSequence,
}

#[derive(
    Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
/// This hash is serialized as a byte array
pub struct ByteArraySerializer;

#[derive(
    Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
/// This hash is serialized as a base64 string
pub struct Base64Serializer;

#[derive(
    Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
/// This hash is serialized as a byte sequence (rather than a byte array).
///
/// Byte arrays are generally more compact than sequences, but not all formats
/// support them, JSON included. For those formats, you can use this serialization
/// method, or Base64 strings.
pub struct ByteSequenceSerializer;

impl HashSerializer for ByteArraySerializer {
    fn get() -> HashSerialization {
        HashSerialization::ByteArray
    }
}

impl HashSerializer for Base64Serializer {
    fn get() -> HashSerialization {
        HashSerialization::Base64
    }
}

impl HashSerializer for ByteSequenceSerializer {
    fn get() -> HashSerialization {
        HashSerialization::ByteSequence
    }
}

/// Length of the prefix bytes (3)
pub const HOLO_HASH_PREFIX_LEN: usize = 3;

/// Length of the core bytes (32)
pub const HOLO_HASH_CORE_LEN: usize = 32;

/// Length of the location bytes (4)
pub const HOLO_HASH_LOC_LEN: usize = 4;

/// Length of the core bytes + the loc bytes (36 = 32 + 4),
/// i.e. everything except the type prefix
pub const HOLO_HASH_UNTYPED_LEN: usize = HOLO_HASH_CORE_LEN + HOLO_HASH_LOC_LEN; // 36

/// Length of the full HoloHash bytes (39 = 3 + 32 + 4)
pub const HOLO_HASH_FULL_LEN: usize = HOLO_HASH_PREFIX_LEN + HOLO_HASH_CORE_LEN + HOLO_HASH_LOC_LEN;

/// Helper for ensuring the the proper number of bytes is used in various situations
#[macro_export]
macro_rules! assert_length {
    ($len:expr, $hash:expr) => {
        debug_assert_eq!(
            $hash.len(),
            $len,
            "invalid byte count for HoloHash {:?}",
            $hash
        );
    };
}

/// A HoloHash contains a vector of 36 bytes representing a 32-byte blake2b hash
/// plus 4 bytes representing a DHT location. It also contains a zero-sized
/// type which specifies what it is a hash of.
///
/// There is custom de/serialization implemented in [ser.rs]
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct HoloHash<T: HashType, S: HashSerializer = ByteArraySerializer> {
    pub(crate) hash: Vec<u8>,
    pub(crate) hash_type: T,
    pub(crate) _serializer: std::marker::PhantomData<S>,
}

#[cfg(feature = "fuzzing")]
impl<'a, P: PrimitiveHashType, S: HashSerializer> arbitrary::Arbitrary<'a> for HoloHash<P, S> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let mut buf = [0; HOLO_HASH_FULL_LEN];
        buf[0..HOLO_HASH_PREFIX_LEN].copy_from_slice(P::static_prefix());
        buf[HOLO_HASH_PREFIX_LEN..]
            .copy_from_slice(u.bytes(HOLO_HASH_FULL_LEN - HOLO_HASH_PREFIX_LEN)?);
        Ok(HoloHash {
            hash: buf.to_vec(),
            hash_type: P::new(),
            _serializer: std::marker::PhantomData,
        })
    }
}

#[cfg(feature = "fuzzing")]
impl<T: HashType + proptest::arbitrary::Arbitrary, S> proptest::arbitrary::Arbitrary
    for HoloHash<T, S>
where
    T::Strategy: 'static,
    S: HashSerializer,
{
    type Parameters = ();
    type Strategy = proptest::strategy::BoxedStrategy<HoloHash<T, S>>;

    fn arbitrary_with((): Self::Parameters) -> Self::Strategy {
        use proptest::strategy::Strategy;

        let strat = T::arbitrary().prop_flat_map(move |hash_type| {
            let gen_strat = proptest::string::bytes_regex(r".[39]").unwrap();
            gen_strat.prop_map(move |mut buf| {
                assert_eq!(buf.len(), 39);
                buf[0..HOLO_HASH_PREFIX_LEN].copy_from_slice(hash_type.get_prefix());
                HoloHash {
                    hash: buf.to_vec(),
                    hash_type,
                    _serializer: std::marker::PhantomData,
                }
            })
        });
        strat.boxed()
    }
}

impl<T: HashType, S: HashSerializer> HoloHash<T, S> {
    /// Change the type of this HoloHash, keeping the same bytes
    pub(crate) fn retype<TT: HashType>(mut self, hash_type: TT) -> HoloHash<TT, S> {
        let prefix = hash_type.get_prefix();
        self.hash[0..HOLO_HASH_PREFIX_LEN].copy_from_slice(&prefix[0..HOLO_HASH_PREFIX_LEN]);
        HoloHash {
            hash: self.hash,
            hash_type,
            _serializer: std::marker::PhantomData,
        }
    }

    /// The HashType of this hash
    pub fn hash_type(&self) -> &T {
        &self.hash_type
    }

    /// Get the raw 39-byte Vec including the 3 byte prefix, base 32 bytes, and the 4 byte loc
    pub fn get_raw_39(&self) -> &[u8] {
        &self.hash[..]
    }

    /// Get 36-byte Vec which excludes the 3 byte prefix
    pub fn get_raw_36(&self) -> &[u8] {
        let bytes = &self.hash[HOLO_HASH_PREFIX_LEN..];
        assert_length!(HOLO_HASH_UNTYPED_LEN, bytes);
        bytes
    }

    /// Fetch just the core 32 bytes (without the 4 location bytes)
    pub fn get_raw_32(&self) -> &[u8] {
        let bytes = &self.hash[HOLO_HASH_PREFIX_LEN..HOLO_HASH_PREFIX_LEN + HOLO_HASH_CORE_LEN];
        assert_length!(HOLO_HASH_CORE_LEN, bytes);
        bytes
    }

    /// Fetch the holo dht location for this hash
    pub fn get_loc(&self) -> DhtLocation {
        DhtLocation::new(bytes_to_loc(
            &self.hash[HOLO_HASH_FULL_LEN - HOLO_HASH_LOC_LEN..],
        ))
    }

    /// consume into the inner byte vector
    pub fn into_inner(self) -> Vec<u8> {
        assert_length!(HOLO_HASH_FULL_LEN, &self.hash);
        self.hash
    }

    /// Get the hex representation of the hash bytes
    pub fn to_hex(&self) -> String {
        holochain_util::hex::bytes_to_hex(&self.hash, false)
    }
}

impl<T: HashType> HoloHash<T> {
    /// Construct a HoloHash from a 32-byte hash.
    /// The 3 prefix bytes will be added based on the provided HashType,
    /// and the 4 location bytes will be computed.
    #[cfg(feature = "hashing")]
    pub fn from_raw_32_and_type(mut hash: Vec<u8>, hash_type: T) -> Self {
        assert_length!(HOLO_HASH_CORE_LEN, &hash);
        hash.append(&mut encode::holo_dht_location_bytes(&hash));
        assert_length!(HOLO_HASH_UNTYPED_LEN, &hash);

        HoloHash::from_raw_36_and_type(hash, hash_type)
    }

    /// Raw constructor: Create a HoloHash from 39 bytes, using the prefix
    /// bytes to determine the hash_type
    pub fn from_raw_39(hash: Vec<u8>) -> HoloHashResult<Self> {
        assert_length!(HOLO_HASH_FULL_LEN, &hash);
        let hash_type = T::try_from_prefix(&hash[0..3])?;
        Ok(Self {
            hash,
            hash_type,
            _serializer: std::marker::PhantomData,
        })
    }

    /// Raw constructor: Create a HoloHash from 39 bytes, using the prefix
    /// bytes to determine the hash_type. Panics if hash_type does not match.
    pub fn from_raw_39_panicky(hash: Vec<u8>) -> Self {
        Self::from_raw_39(hash).expect("the specified hash_type does not match the prefix bytes")
    }

    /// Use a precomputed hash + location byte array in vec form,
    /// along with a type, to construct a hash. Used in this crate only, for testing.
    pub fn from_raw_36_and_type(mut bytes: Vec<u8>, hash_type: T) -> Self {
        assert_length!(HOLO_HASH_UNTYPED_LEN, &bytes);
        let mut hash = hash_type.get_prefix().to_vec();
        hash.append(&mut bytes);
        assert_length!(HOLO_HASH_FULL_LEN, &hash);
        Self {
            hash,
            hash_type,
            _serializer: std::marker::PhantomData,
        }
    }
}

impl<P: PrimitiveHashType> HoloHash<P> {
    /// Construct from 36 raw bytes, using the known PrimitiveHashType
    pub fn from_raw_36(hash: Vec<u8>) -> Self {
        assert_length!(HOLO_HASH_UNTYPED_LEN, &hash);
        Self::from_raw_36_and_type(hash, P::new())
    }

    #[cfg(feature = "hashing")]
    /// Construct a HoloHash from a prehashed raw 32-byte slice.
    /// The location bytes will be calculated.
    pub fn from_raw_32(hash: Vec<u8>) -> Self {
        Self::from_raw_32_and_type(hash, P::new())
    }
}

impl<T: HashType, S: HashSerializer> AsRef<[u8]> for HoloHash<T, S> {
    fn as_ref(&self) -> &[u8] {
        assert_length!(HOLO_HASH_FULL_LEN, &self.hash);
        &self.hash
    }
}

#[cfg(feature = "rusqlite")]
impl<T: HashType, S: HashSerializer> rusqlite::ToSql for HoloHash<T, S> {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Borrowed(self.as_ref().into()))
    }
}

#[cfg(feature = "rusqlite")]
impl<T: HashType, S: HashSerializer> rusqlite::types::FromSql for HoloHash<T, S> {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        Vec::<u8>::column_result(value).and_then(|bytes| {
            HoloHash::from_raw_39(bytes)
                .map(|h| h.change_serialization())
                .map_err(|_| rusqlite::types::FromSqlError::InvalidType)
        })
    }
}

impl<T: HashType, S: HashSerializer> IntoIterator for HoloHash<T, S> {
    type Item = u8;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.hash.into_iter()
    }
}

impl<T: HashType, S: HashSerializer> HasHash<T, S> for HoloHash<T, S> {
    fn as_hash(&self) -> &HoloHash<T, S> {
        self
    }
    fn into_hash(self) -> HoloHash<T, S> {
        self
    }
}

// NB: See encode/encode_raw module for Display impl
impl<T: HashType, S: HashSerializer> std::fmt::Debug for HoloHash<T, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}({})", self.hash_type().hash_name(), self))?;
        Ok(())
    }
}

/// internal convert 4 location bytes into a u32 location
fn bytes_to_loc(bytes: &[u8]) -> u32 {
    (bytes[0] as u32)
        + ((bytes[1] as u32) << 8)
        + ((bytes[2] as u32) << 16)
        + ((bytes[3] as u32) << 24)
}

#[cfg(test)]
mod tests {
    use crate::{hash::HashSerializer, *};

    fn assert_type<T: HashType, S: HashSerializer>(t: &str, h: HoloHash<T, S>) {
        assert_eq!(3_688_618_971, h.get_loc().as_u32());
        assert_eq!(h.hash_type().hash_name(), t);
        assert_eq!(
            "[219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219]",
            format!("{:?}", h.get_raw_32()),
        );
    }

    #[test]
    fn test_enum_types() {
        assert_type(
            "DnaHash",
            DnaHash::from_raw_36(vec![0xdb; HOLO_HASH_UNTYPED_LEN]),
        );
        assert_type(
            "NetIdHash",
            NetIdHash::from_raw_36(vec![0xdb; HOLO_HASH_UNTYPED_LEN]),
        );
        assert_type(
            "AgentPubKey",
            AgentPubKey::from_raw_36(vec![0xdb; HOLO_HASH_UNTYPED_LEN]),
        );
        assert_type(
            "EntryHash",
            EntryHash::from_raw_36(vec![0xdb; HOLO_HASH_UNTYPED_LEN]),
        );
        assert_type(
            "DhtOpHash",
            DhtOpHash::from_raw_36(vec![0xdb; HOLO_HASH_UNTYPED_LEN]),
        );
        assert_type(
            "ExternalHash",
            ExternalHash::from_raw_36(vec![0xdb; HOLO_HASH_UNTYPED_LEN]),
        );
    }

    #[test]
    #[should_panic]
    fn test_fails_with_bad_size() {
        DnaHash::from_raw_36(vec![0xdb; 35]);
    }
}
