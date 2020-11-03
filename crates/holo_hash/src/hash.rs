use crate::{has_hash::HasHash, HashType, PrimitiveHashType};

pub(crate) const HASH_PREFIX_LEN: usize = 3;
pub(crate) const HASH_CORE_LEN: usize = 32;
pub(crate) const HASH_LOC_LEN: usize = 4;

/// Length of the core bytes + the loc bytes (36 = 32 + 4)
pub const HOLO_HASH_FULL_LEN: usize = HASH_CORE_LEN + HASH_LOC_LEN; // 36

/// Length of the full HoloHash bytes (39 = 3 + 32 + 4)
pub const HOLO_HASH_SERIALIZED_LEN: usize = HASH_PREFIX_LEN + HASH_CORE_LEN + HASH_LOC_LEN;

/// Alias for `HOLO_HASH_SERIALIZED_LEN`
pub const HOLO_HASH_RAW_LEN: usize = HOLO_HASH_SERIALIZED_LEN;

/// A HoloHash contains a vector of 36 bytes representing a 32-byte blake2b hash
/// plus 4 bytes representing a DHT location. It also contains a zero-sized
/// type which specifies what it is a hash of.
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct HoloHash<T: HashType> {
    hash: Vec<u8>,
    hash_type: T,
}

impl<T: HashType> HoloHash<T> {
    /// Raw constructor: use a precomputed hash + location byte array in vec
    /// form, along with a type, to construct a hash.
    pub fn from_full_bytes_and_type(mut bytes: Vec<u8>, hash_type: T) -> Self {
        let mut hash = hash_type.get_prefix().to_vec();
        hash.append(&mut bytes);
        assert_length(HOLO_HASH_RAW_LEN, &hash);
        Self { hash, hash_type }
    }

    /// Change the type of this HoloHash, keeping the same bytes
    pub(crate) fn retype<TT: HashType>(mut self, hash_type: TT) -> HoloHash<TT> {
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

    /// Get the raw byte array including the 3 byte prefix, base 32 bytes, and the 4 byte loc
    pub fn get_raw_bytes(&self) -> &[u8] {
        &self.hash[..]
    }

    /// Get the full byte array including the base 32 bytes and the 4 byte loc
    #[deprecated = "no need for full bytes anymore"]
    pub fn get_full_bytes(&self) -> &[u8] {
        &self.hash[HASH_PREFIX_LEN..]
    }

    /// Fetch just the core 32 bytes (without the 4 location bytes)
    // TODO: change once prefix is included [ B-02112 ]
    #[deprecated = "is there a need for core bytes anymore?"]
    pub fn get_core_bytes(&self) -> &[u8] {
        let bytes = &self.hash[HASH_PREFIX_LEN..HASH_PREFIX_LEN + HASH_CORE_LEN];
        assert_length(HASH_CORE_LEN, bytes);
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
    pub fn from_full_bytes(hash: Vec<u8>) -> Self {
        assert_length(HOLO_HASH_FULL_LEN, &hash);
        Self::from_full_bytes_and_type(hash, P::new())
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
            format!("{:?}", h.get_bytes()),
        );
        assert_eq!(
            format!(
                "{}(0xdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdb)",
                t
            ),
            format!("{:?}", h),
        );
    }

    #[test]
    #[cfg(not(feature = "string-encoding"))]
    fn test_enum_types() {
        assert_type("DnaHash", DnaHash::from_full_bytes(vec![0xdb; 36]));
        assert_type("NetIdHash", NetIdHash::from_full_bytes(vec![0xdb; 36]));
        assert_type("AgentPubKey", AgentPubKey::from_full_bytes(vec![0xdb; 36]));
        assert_type("EntryHash", EntryHash::from_full_bytes(vec![0xdb; 36]));
        assert_type("DhtOpHash", DhtOpHash::from_full_bytes(vec![0xdb; 36]));
    }

    #[test]
    #[should_panic]
    fn test_fails_with_bad_size() {
        DnaHash::from_full_bytes(vec![0xdb; 35]);
    }
}
