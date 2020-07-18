use crate::{has_hash::HasHash, HashType, PrimitiveHashType};

// TODO: make holochain_serial! / the derive able to deal with a type param
// or if not, implement the TryFroms manually...
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct HoloHashImpl<T> {
    #[serde(with = "serde_bytes")]
    hash: Vec<u8>,

    hash_type: T,
}

impl<T: HashType> HoloHashImpl<T> {
    /// Raw constructor: use a precomputed hash + location byte array in vec
    /// form, along with a type, to construct a hash.
    pub fn from_raw_bytes_and_type(hash: Vec<u8>, hash_type: T) -> Self {
        if hash.len() != 36 {
            panic!(
                "invalid holo_hash byte count, expected: 36, found: {}. {:?}",
                hash.len(),
                &hash
            );
        }
        Self { hash, hash_type }
    }

    /// Change the type of this HoloHash, keeping the same bytes
    pub fn retype<TT: HashType>(self, hash_type: TT) -> HoloHashImpl<TT> {
        HoloHashImpl {
            hash: self.hash,
            hash_type,
        }
    }

    pub fn hash_type(&self) -> &T {
        &self.hash_type
    }

    /// Get the full byte array including the base 32 bytes and the 4 byte loc
    pub fn get_raw(&self) -> &[u8] {
        &self.hash
    }

    /// Fetch just the core 32 bytes (without the 4 location bytes)
    pub fn get_bytes(&self) -> &[u8] {
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

impl<P: PrimitiveHashType> HoloHashImpl<P> {
    pub fn from_raw_bytes(hash: Vec<u8>) -> Self {
        Self::from_raw_bytes_and_type(hash, P::new())
    }
}

impl<T: HashType> AsRef<[u8]> for HoloHashImpl<T> {
    fn as_ref(&self) -> &[u8] {
        &self.hash[0..32]
    }
}

impl<T: HashType> HasHash<T> for HoloHashImpl<T> {
    fn hash(&self) -> &HoloHashImpl<T> {
        &self
    }
    fn into_hash(self) -> HoloHashImpl<T> {
        self
    }
}

// NB: See encode/encode_raw module for Display impl
impl<T: HashType> std::fmt::Debug for HoloHashImpl<T> {
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

    #[cfg(not(feature = "string-encoding"))]
    fn assert_type<T: HashType>(t: &str, h: HoloHashImpl<T>) {
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
        assert_type(
            "EntryContentHash",
            EntryContentHash::from_raw_bytes(vec![0xdb; 36]),
        );
        assert_type("DhtOpHash", DhtOpHash::from_raw_bytes(vec![0xdb; 36]));
    }

    #[test]
    #[should_panic]
    fn test_fails_with_bad_size() {
        DnaHash::from_raw_bytes(vec![0xdb; 35]);
    }

    #[test]
    fn test_serialized_bytes_roundtrip() {
        use holochain_serialized_bytes::SerializedBytes;
        use std::convert::TryInto;

        let h_orig = DnaHash::from_raw_bytes(vec![0xdb; 36]);
        let h: SerializedBytes = h_orig.clone().try_into().unwrap();
        let h: DnaHash = h.try_into().unwrap();

        assert_eq!(h_orig, h);
        assert_eq!(h.hash_type, hash_type::Dna::new());
    }

    #[test]
    fn test_rmp_roundtrip() {
        let h_orig = AgentPubKey::from_raw_bytes(vec![0xdb; 36]);
        let buf = rmp_serde::to_vec_named(&h_orig).unwrap();
        let h: AgentPubKey = rmp_serde::from_read_ref(&buf).unwrap();

        assert_eq!(h_orig, h);
        assert_eq!(h.hash_type, hash_type::Agent::new());
    }

    #[test]
    fn test_entry_hash_roundtrip() {
        {
            let h_orig =
                EntryHash::from_raw_bytes_and_type(vec![0xdb; 36], hash_type::Entry::Content);
            let buf = rmp_serde::to_vec_named(&h_orig).unwrap();
            let h: EntryHash = rmp_serde::from_read_ref(&buf).unwrap();

            assert_eq!(h_orig, h);
            assert_eq!(h.hash_type, hash_type::Entry::Content);
        }
        {
            let h_orig =
                EntryHash::from_raw_bytes_and_type(vec![0xdb; 36], hash_type::Entry::Agent);
            let buf = rmp_serde::to_vec_named(&h_orig).unwrap();
            let h: EntryHash = rmp_serde::from_read_ref(&buf).unwrap();

            assert_eq!(h_orig, h);
            assert_eq!(h.hash_type, hash_type::Entry::Agent);
        }
    }

    #[test]
    fn test_struct_roundtrip() {
        #[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        struct Data {
            a: AgentPubKey,
            h: HeaderHash,
        }

        let orig = Data {
            a: AgentPubKey::from_raw_bytes(vec![0xdb; 36]),
            h: HeaderHash::from_raw_bytes(vec![0xdb; 36]),
        };

        let buf = rmp_serde::to_vec_named(&orig).unwrap();
        let res: Data = rmp_serde::from_read_ref(&buf).unwrap();

        assert_eq!(orig, res);
        assert_eq!(orig.a.hash_type, hash_type::Agent::new());
        assert_eq!(orig.h.hash_type, hash_type::Header::new());
    }

    #[test]
    fn test_generic_content_roundtrip() {
        #[derive(Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        struct Generic<K> {
            bytes: Vec<u8>,
            __marker: std::marker::PhantomData<K>,
        }

        impl<K> Generic<K>
        where
            K: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug,
            // V: Serialize + DeserializeOwned + std::fmt::Debug,
        {
            fn new() -> Self {
                Self {
                    bytes: Vec::new(),
                    __marker: Default::default(),
                }
            }

            fn get(&self) -> K {
                rmp_serde::from_read_ref(&self.bytes).unwrap()
            }

            fn put(&mut self, k: &K) {
                self.bytes = rmp_serde::to_vec_named(k).unwrap();
            }
        }

        let mut g: Generic<HeaderHash> = Generic::new();
        let h = HeaderHash::from_raw_bytes(vec![0xdb; 36]);
        g.put(&h);
        assert_eq!(h, g.get());
    }
}
