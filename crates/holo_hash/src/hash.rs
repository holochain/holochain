use crate::{error::HoloHashResult, has_hash::HasHash, HashType, PrimitiveHashType};

/// Length of the prefix bytes (3)
pub const HOLO_HASH_PREFIX_LEN: usize = 3;

/// Length of the core bytes (32)
pub const HOLO_HASH_CORE_LEN: usize = 32;

/// Length of the location bytes (4)
pub const HOLO_HASH_LOC_LEN: usize = 4;

/// Length of the core bytes + the loc bytes (36 = 32 + 4)
pub const HOLO_HASH_FULL_LEN: usize = HOLO_HASH_CORE_LEN + HOLO_HASH_LOC_LEN; // 36

/// Length of the full HoloHash bytes (39 = 3 + 32 + 4)
pub const HOLO_HASH_RAW_LEN: usize = HOLO_HASH_PREFIX_LEN + HOLO_HASH_CORE_LEN + HOLO_HASH_LOC_LEN;

/// A HoloHash contains a vector of 36 bytes representing a 32-byte blake2b hash
/// plus 4 bytes representing a DHT location. It also contains a zero-sized
/// type which specifies what it is a hash of.
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct HoloHash<T: HashType> {
    hash: Vec<u8>,
    hash_type: T,
}

impl<T: HashType> HoloHash<T> {
    /// Raw constructor: Create a HoloHash from 39 bytes, using the prefix
    /// bytes to determine the hash_type
    pub fn from_raw_39(hash: Vec<u8>) -> HoloHashResult<Self> {
        assert_length(HOLO_HASH_RAW_LEN, &hash);
        let hash_type = T::try_from_prefix(&hash[0..3])?;
        Ok(Self { hash, hash_type })
    }
    /// Raw constructor: Create a HoloHash from 39 bytes, using the prefix
    /// bytes to determine the hash_type. Panics if hash_type does not match.
    pub fn from_raw_39_panicky(hash: Vec<u8>) -> Self {
        Self::from_raw_39(hash).expect("the specified hash_type does not match the prefix bytes")
    }

    /// Use a precomputed hash + location byte array in vec form,
    /// along with a type, to construct a hash. Used in this crate only, for testing.
    pub fn from_raw_36_and_type(mut bytes: Vec<u8>, hash_type: T) -> Self {
        assert_length(HOLO_HASH_FULL_LEN, &bytes);
        let mut hash = hash_type.get_prefix().to_vec();
        hash.append(&mut bytes);
        assert_length(HOLO_HASH_RAW_LEN, &hash);
        Self { hash, hash_type }
    }

    /// Change the type of this HoloHash, keeping the same bytes
    pub fn retype<TT: HashType>(mut self, hash_type: TT) -> HoloHash<TT> {
        let prefix = hash_type.get_prefix();
        let _ = std::mem::replace(&mut self.hash[0], prefix[0]);
        let _ = std::mem::replace(&mut self.hash[1], prefix[1]);
        let _ = std::mem::replace(&mut self.hash[2], prefix[2]);
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
        assert_length(HOLO_HASH_FULL_LEN, bytes);
        bytes
    }

    /// Fetch just the core 32 bytes (without the 4 location bytes)
    pub fn get_raw_32(&self) -> &[u8] {
        let bytes = &self.hash[HOLO_HASH_PREFIX_LEN..HOLO_HASH_PREFIX_LEN + HOLO_HASH_CORE_LEN];
        assert_length(HOLO_HASH_CORE_LEN, bytes);
        bytes
    }

    /// Fetch the holo dht location for this hash
    pub fn get_loc(&self) -> u32 {
        bytes_to_loc(&self.hash[HOLO_HASH_RAW_LEN - 4..])
    }

    /// consume into the inner byte vector
    pub fn into_inner(self) -> Vec<u8> {
        assert_length(HOLO_HASH_RAW_LEN, &self.hash);
        self.hash
    }
}

impl<P: PrimitiveHashType> HoloHash<P> {
    /// Construct from 36 raw bytes, using the known PrimitiveHashType
    pub fn from_raw_36(hash: Vec<u8>) -> Self {
        assert_length(HOLO_HASH_FULL_LEN, &hash);
        Self::from_raw_36_and_type(hash, P::new())
    }
}

impl<T: HashType> AsRef<[u8]> for HoloHash<T> {
    // TODO: revisit this, especially after changing serialization format. [ B-02112 ]
    // Should this be 32, 36, or 39 bytes?
    fn as_ref(&self) -> &[u8] {
        assert_length(HOLO_HASH_RAW_LEN, &self.hash);
        &self.hash
    }
}

impl<T: HashType> IntoIterator for HoloHash<T> {
    type Item = u8;
    type IntoIter = std::iter::Take<std::vec::IntoIter<Self::Item>>;
    // TODO: revisit this, especially after changing serialization format. [ B-02112 ]
    // Should this be 32, 36, or 39 bytes?
    fn into_iter(self) -> Self::IntoIter {
        self.hash.into_iter().take(HOLO_HASH_FULL_LEN)
    }
}

impl<T: HashType> HasHash<T> for HoloHash<T> {
    fn as_hash(&self) -> &HoloHash<T> {
        &self
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

/// Helper for ensuring the the proper number of bytes is used in various situations
pub fn assert_length(len: usize, hash: &[u8]) {
    if hash.len() != len {
        panic!(
            "invalid holo_hash byte count, expected: {}, found: {}. {:?}",
            len,
            hash.len(),
            hash
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[cfg(not(feature = "string-encoding"))]
    fn assert_type<T: HashType>(t: &str, h: HoloHash<T>) {
        assert_eq!(3_688_618_971, h.get_loc());
        assert_eq!(
            "[219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219]",
            format!("{:?}", h.get_raw_32()),
        );
    }

    #[test]
    #[cfg(not(feature = "string-encoding"))]
    fn test_enum_types() {
        assert_type("DnaHash", DnaHash::from_raw_36(vec![0xdb; 36]));
        assert_type("NetIdHash", NetIdHash::from_raw_36(vec![0xdb; 36]));
        assert_type("AgentPubKey", AgentPubKey::from_raw_36(vec![0xdb; 36]));
        assert_type("EntryHash", EntryHash::from_raw_36(vec![0xdb; 36]));
        assert_type("DhtOpHash", DhtOpHash::from_raw_36(vec![0xdb; 36]));
    }

    #[test]
    #[should_panic]
    fn test_fails_with_bad_size() {
        DnaHash::from_raw_36(vec![0xdb; 35]);
    }
}
