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

#[cfg(feature = "hashing")]
use crate::encode;
use crate::error::{HoloHashError, HoloHashResult};
use crate::has_hash::HasHash;
use crate::HashType;
use crate::PrimitiveHashType;

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

/// Helper for ensuring the proper number of bytes is used in various situations
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
pub struct HoloHash<T: HashType> {
    hash: Vec<u8>,
    hash_type: T,
}

impl<T: HashType> HoloHash<T> {
    /// Raw constructor: Create a HoloHash from 39 bytes, using the prefix
    /// bytes to determine the hash_type
    pub fn try_from_raw_39(hash: Vec<u8>) -> HoloHashResult<Self> {
        if hash.len() != HOLO_HASH_FULL_LEN {
            return Err(HoloHashError::BadSize);
        }
        let hash_type = T::try_from_prefix(&hash[0..3])?;
        Ok(Self { hash, hash_type })
    }

    /// Raw constructor: Create a HoloHash from 39 bytes, using the prefix
    /// bytes to determine the hash_type.
    /// Panics if hash_type does not match or hash is incorrect length.
    pub fn from_raw_39(hash: Vec<u8>) -> Self {
        Self::try_from_raw_39(hash).unwrap()
    }

    /// Use a precomputed hash + location byte array in vec form,
    /// along with a type, to construct a hash.
    pub fn try_from_raw_36_and_type(mut bytes: Vec<u8>, hash_type: T) -> HoloHashResult<Self> {
        if bytes.len() != HOLO_HASH_UNTYPED_LEN {
            return Err(HoloHashError::BadSize);
        }
        let mut hash = hash_type.get_prefix().to_vec();
        hash.append(&mut bytes);
        Ok(Self { hash, hash_type })
    }

    /// Use a precomputed hash + location byte array in vec form,
    /// along with a type, to construct a hash.
    /// Panics hash is incorrect length.
    pub fn from_raw_36_and_type(bytes: Vec<u8>, hash_type: T) -> Self {
        Self::try_from_raw_36_and_type(bytes, hash_type).unwrap()
    }

    /// Change the type of this HoloHash, keeping the same bytes
    pub(crate) fn retype<TT: HashType>(mut self, hash_type: TT) -> HoloHash<TT> {
        let prefix = hash_type.get_prefix();
        self.hash[0..HOLO_HASH_PREFIX_LEN].copy_from_slice(&prefix[0..HOLO_HASH_PREFIX_LEN]);
        HoloHash {
            hash: self.hash,
            hash_type,
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
    pub fn get_loc(&self) -> u32 {
        bytes_to_loc(&self.hash[HOLO_HASH_FULL_LEN - HOLO_HASH_LOC_LEN..])
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

#[cfg(feature = "kitsune2")]
impl crate::DnaHash {
    /// Convert this holo hash into a kitsune2 SpaceId.
    pub fn to_k2_space(&self) -> kitsune2_api::SpaceId {
        kitsune2_api::SpaceId::from(bytes::Bytes::copy_from_slice(self.get_raw_32()))
    }

    /// Convert a kitsune2 SpaceId into a DnaHash.
    #[cfg(feature = "hashing")]
    pub fn from_k2_space(space: &kitsune2_api::SpaceId) -> Self {
        Self::from_raw_32(space.to_vec())
    }
}

#[cfg(feature = "kitsune2")]
impl crate::AgentPubKey {
    /// Convert this holo hash into a kitsune2 AgentId.
    pub fn to_k2_agent(&self) -> kitsune2_api::AgentId {
        kitsune2_api::AgentId::from(bytes::Bytes::copy_from_slice(self.get_raw_32()))
    }

    /// Convert a kitsune2 AgentId into a AgentPubKey.
    #[cfg(feature = "hashing")]
    pub fn from_k2_agent(agent: &kitsune2_api::AgentId) -> Self {
        Self::from_raw_32(agent.to_vec())
    }
}

#[cfg(feature = "kitsune2")]
impl crate::DhtOpHash {
    /// Convert this holo hash into a kitsune2 OpId.
    #[deprecated(since = "0.5.5", note = "Use `to_located_k2_op_id` instead")]
    pub fn to_k2_op(&self) -> kitsune2_api::OpId {
        kitsune2_api::OpId::from(bytes::Bytes::copy_from_slice(self.get_raw_36()))
    }

    /// Convert this [`DhtOpHash`](crate::DhtOpHash) into a kitsune2 [`OpId`](kitsune2_api::OpId),
    /// with appended location bytes.
    ///
    /// The location that is reported by a [`DhtOpHash`](crate::DhtOpHash) is the location of the
    /// op itself but that is not the location of the op in the DHT. The latter is given by the
    /// [`OpBasis`](crate::OpBasis).
    ///
    /// The resulting [`OpId`](kitsune2_api::OpId) will have the first 32 bytes as the core hash of
    /// the op, and the last 4 bytes as the location bytes taken from the
    /// [`OpBasis`](crate::OpBasis).
    pub fn to_located_k2_op_id(&self, op_basis: &crate::OpBasis) -> kitsune2_api::OpId {
        let mut inner = bytes::BytesMut::with_capacity(HOLO_HASH_UNTYPED_LEN);
        inner.extend_from_slice(self.get_raw_32());
        inner.extend_from_slice(&op_basis.get_raw_36()[HOLO_HASH_CORE_LEN..]);
        kitsune2_api::OpId::from(inner.freeze())
    }

    /// Convert a kitsune2 OpId into a DhtOpHash.
    #[cfg(feature = "hashing")]
    pub fn from_k2_op(op: &kitsune2_api::OpId) -> Self {
        Self::from_raw_32(op[..HOLO_HASH_CORE_LEN].to_vec())
    }
}

#[cfg(feature = "hashing")]
impl<T: HashType> HoloHash<T> {
    /// Construct a HoloHash from a 32-byte hash.
    /// The 3 prefix bytes will be added based on the provided HashType,
    /// and the 4 location bytes will be computed.
    pub fn from_raw_32_and_type(mut hash: Vec<u8>, hash_type: T) -> Self {
        assert_length!(HOLO_HASH_CORE_LEN, &hash);
        hash.append(&mut encode::holo_dht_location_bytes(&hash));
        assert_length!(HOLO_HASH_UNTYPED_LEN, &hash);

        HoloHash::from_raw_36_and_type(hash, hash_type)
    }
}

impl<P: PrimitiveHashType> HoloHash<P> {
    /// Construct from 36 raw bytes, using the known PrimitiveHashType
    pub fn from_raw_36(hash: Vec<u8>) -> Self {
        assert_length!(HOLO_HASH_UNTYPED_LEN, &hash);
        Self::from_raw_36_and_type(hash, P::new())
    }

    /// Construct a HoloHash from a prehashed raw 32-byte slice.
    /// The location bytes will be calculated.
    #[cfg(feature = "hashing")]
    pub fn from_raw_32(hash: Vec<u8>) -> Self {
        Self::from_raw_32_and_type(hash, P::new())
    }
}

impl<T: HashType> AsRef<[u8]> for HoloHash<T> {
    fn as_ref(&self) -> &[u8] {
        assert_length!(HOLO_HASH_FULL_LEN, &self.hash);
        &self.hash
    }
}

#[cfg(feature = "rusqlite")]
impl<T: HashType> rusqlite::ToSql for HoloHash<T> {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Borrowed(self.as_ref().into()))
    }
}

#[cfg(feature = "rusqlite")]
impl<T: HashType> rusqlite::types::FromSql for HoloHash<T> {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        Vec::<u8>::column_result(value).and_then(|bytes| {
            Self::try_from_raw_39(bytes).map_err(|_| rusqlite::types::FromSqlError::InvalidType)
        })
    }
}

impl<T: HashType> IntoIterator for HoloHash<T> {
    type Item = u8;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.hash.into_iter()
    }
}

impl<T: HashType> HasHash for HoloHash<T> {
    type HashType = T;

    fn as_hash(&self) -> &HoloHash<T> {
        self
    }
    fn into_hash(self) -> HoloHash<T> {
        self
    }
}

// NB: See encode/encode_raw module for Display impl
impl<T: HashType> std::fmt::Debug for HoloHash<T> {
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
    use crate::*;

    fn assert_type<T: HashType>(t: &str, h: HoloHash<T>) {
        assert_eq!(3_688_618_971, h.get_loc());
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
    fn test_from_raw_36_panics_with_bad_size() {
        DnaHash::from_raw_36(vec![0xdb; 35]);
    }

    #[test]
    fn test_try_from_raw_39_errors_with_bad_size() {
        let mut raw = vec![132, 45, 36];
        raw.extend(vec![0xdb; 35]);

        let res = DnaHash::try_from_raw_39(raw);
        assert_eq!(res, Err(HoloHashError::BadSize));
    }

    #[test]
    fn test_try_from_raw_39_errors_with_bad_prefix() {
        let res = DnaHash::try_from_raw_39(vec![0xdb; 39]);
        assert!(matches!(res, Err(HoloHashError::BadPrefix { .. })));
    }

    #[test]
    #[should_panic]
    fn test_from_raw_39_panics_with_bad_size() {
        let mut raw = vec![132, 45, 36];
        raw.extend(vec![0xdb; 35]);

        DnaHash::from_raw_39(raw);
    }

    #[test]
    #[should_panic]
    fn test_from_raw_39_panics_with_bad_prefix() {
        DnaHash::from_raw_39(vec![0xdb; 39]);
    }

    #[test]
    fn test_try_from_raw_36_and_type_errors_with_bad_size() {
        let res = HoloHash::try_from_raw_36_and_type(vec![0xdb; 35], hash_type::Dna);
        assert_eq!(res, Err(HoloHashError::BadSize));
    }

    #[test]
    #[should_panic]
    fn test_from_raw_36_and_type_panics_with_bad_size() {
        HoloHash::from_raw_36_and_type(vec![0xdb; 35], hash_type::Dna);
    }

    #[test]
    fn test_try_from_raw_36_and_type() {
        let res = HoloHash::try_from_raw_36_and_type(vec![0xdb; 36], hash_type::Dna);
        assert!(res.is_ok());
    }

    #[test]
    fn test_from_raw_36_and_type() {
        HoloHash::from_raw_36_and_type(vec![0xdb; 36], hash_type::Dna);
    }

    #[test]
    fn test_try_from_raw_39() {
        let mut raw = vec![132, 45, 36];
        raw.extend(vec![0xdb; 36]);

        let res = DnaHash::try_from_raw_39(raw);
        assert!(res.is_ok());
    }

    #[test]
    fn test_from_raw_39() {
        let mut raw = vec![132, 45, 36];
        raw.extend(vec![0xdb; 36]);

        DnaHash::from_raw_39(raw);
    }
}
