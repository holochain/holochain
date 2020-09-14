use crate::{has_hash::HasHash, HashType, PrimitiveHashType};

pub(crate) const HASH_CORE_LEN: usize = 32;
pub(crate) const HASH_LOC_LEN: usize = 4;

pub(crate) const HASH_SERIALIZED_LEN: usize = HASH_CORE_LEN + HASH_LOC_LEN;

/// A HoloHash contains a vector of 36 bytes representing a 32-byte blake2b hash
/// plus 4 bytes representing a DHT location. It also contains a zero-sized
/// type which specifies what it is a hash of.
// TODO: make holochain_serial! / the derive able to deal with a type param
// or if not, implement the TryFroms manually...
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct HoloHash<T> {
    #[serde(with = "serde_bytes")]
    hash: Vec<u8>,
    hash_type: T,
}

impl<T: HashType> HoloHash<T> {
    /// Raw constructor: use a precomputed hash + location byte array in vec
    /// form, along with a type, to construct a hash.
    pub fn from_raw_bytes_and_type(hash: Vec<u8>, hash_type: T) -> Self {
        assert_length(&hash);
        Self { hash, hash_type }
    }

    /// Change the type of this HoloHash, keeping the same bytes
    pub(crate) fn retype<TT: HashType>(self, hash_type: TT) -> HoloHash<TT> {
        HoloHash {
            hash: self.hash,
            hash_type,
        }
    }

    /// The HashType of this hash
    pub fn hash_type(&self) -> &T {
        &self.hash_type
    }

    /// Get the full byte array including the base 32 bytes and the 4 byte loc
    pub fn get_full_bytes(&self) -> &[u8] {
        &self.hash
    }

    /// Fetch just the core 32 bytes (without the 4 location bytes)
    // TODO: change once prefix is included [ B-02112 ]
    pub fn get_core_bytes(&self) -> &[u8] {
        &self.hash[..self.hash.len() - 4]
    }

    /// Fetch the holo dht location for this hash
    pub fn get_loc(&self) -> u32 {
        bytes_to_loc(&self.hash[self.hash.len() - 4..])
    }

    /// consume into the inner byte vector
    pub fn into_inner(self) -> Vec<u8> {
        self.hash
    }
}

impl<P: PrimitiveHashType> HoloHash<P> {
    /// Construct from 36 raw bytes, using the known PrimitiveHashType
    pub fn from_raw_bytes(hash: Vec<u8>) -> Self {
        Self::from_raw_bytes_and_type(hash, P::new())
    }
}

impl<T: HashType> AsRef<[u8]> for HoloHash<T> {
    // TODO: revisit this, especially after changing serialization format. [ B-02112 ]
    // Should this be 32, 36, or 39 bytes?
    fn as_ref(&self) -> &[u8] {
        &self.hash[..36]
    }
}

impl<T: HashType> IntoIterator for HoloHash<T> {
    type Item = u8;
    type IntoIter = std::iter::Take<std::vec::IntoIter<Self::Item>>;
    // TODO: revisit this, especially after changing serialization format. [ B-02112 ]
    // Should this be 32, 36, or 39 bytes?
    fn into_iter(self) -> Self::IntoIter {
        self.hash.into_iter().take(36)
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

fn assert_length(hash: &[u8]) {
    if hash.len() != HASH_SERIALIZED_LEN {
        panic!(
            "invalid holo_hash byte count, expected: {}, found: {}. {:?}",
            HASH_SERIALIZED_LEN,
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
        assert_type("DnaHash", DnaHash::from_raw_bytes(vec![0xdb; 36]));
        assert_type("NetIdHash", NetIdHash::from_raw_bytes(vec![0xdb; 36]));
        assert_type("AgentPubKey", AgentPubKey::from_raw_bytes(vec![0xdb; 36]));
        assert_type("EntryHash", EntryHash::from_raw_bytes(vec![0xdb; 36]));
        assert_type("DhtOpHash", DhtOpHash::from_raw_bytes(vec![0xdb; 36]));
    }

    #[test]
    #[should_panic]
    fn test_fails_with_bad_size() {
        DnaHash::from_raw_bytes(vec![0xdb; 35]);
    }
}
