use std::convert::TryFrom;

use crate::{
    error::{HoloHashError, HoloHashResult},
    HashType, HoloHash,
};
use holochain_serialized_bytes::{SerializedBytes, SerializedBytesError, UnsafeBytes};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct HoloHash39(#[serde(with = "serde_bytes")] Vec<u8>);
// pub struct HoloHash39([u8; 39]);

impl<T: HashType> TryFrom<HoloHash39> for HoloHash<T> {
    type Error = HoloHashError;

    fn try_from(h: HoloHash39) -> HoloHashResult<Self> {
        if !h.0.len() == 39 {
            Err(HoloHashError::BadSize)
        } else {
            let hash_type = T::try_from_prefix(&h.0[0..3])?;
            let hash = h.0[3..].to_vec();
            Ok(HoloHash::with_pre_hashed_typed(hash, hash_type))
        }
    }
}

impl<T: HashType> From<HoloHash<T>> for HoloHash39 {
    fn from(hash: HoloHash<T>) -> HoloHash39 {
        let mut v = Vec::with_capacity(39);
        v.append(&mut hash.hash_type().get_prefix().to_vec());
        v.append(&mut hash.into_inner());
        HoloHash39(v)
    }
}

impl<T: HashType> std::convert::TryFrom<&HoloHash<T>> for SerializedBytes {
    type Error = SerializedBytesError;
    fn try_from(t: &HoloHash<T>) -> std::result::Result<SerializedBytes, SerializedBytesError> {
        match holochain_serialized_bytes::encode(t) {
            Ok(v) => Ok(SerializedBytes::from(UnsafeBytes::from(v))),
            Err(e) => Err(SerializedBytesError::ToBytes(e.to_string())),
        }
    }
}

impl<T: HashType> std::convert::TryFrom<HoloHash<T>> for SerializedBytes {
    type Error = SerializedBytesError;
    fn try_from(t: HoloHash<T>) -> std::result::Result<SerializedBytes, SerializedBytesError> {
        SerializedBytes::try_from(&t)
    }
}

impl<T: HashType> std::convert::TryFrom<SerializedBytes> for HoloHash<T> {
    type Error = SerializedBytesError;
    fn try_from(sb: SerializedBytes) -> std::result::Result<HoloHash<T>, SerializedBytesError> {
        match holochain_serialized_bytes::decode(sb.bytes()) {
            Ok(v) => Ok(v),
            Err(e) => Err(SerializedBytesError::FromBytes(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use holochain_serialized_bytes::prelude::*;
    use std::convert::TryInto;

    #[test]
    #[cfg(feature = "serialized-bytes")]
    fn test_serialized_bytes_roundtrip() {
        use holochain_serialized_bytes::SerializedBytes;
        use std::convert::TryInto;

        let h_orig = DnaHash::from_raw_bytes(vec![0xdb; 36]);
        let h: SerializedBytes = h_orig.clone().try_into().unwrap();
        let h: DnaHash = h.try_into().unwrap();

        assert_eq!(h_orig, h);
        assert_eq!(*h.hash_type(), hash_type::Dna::new());
    }

    #[test]
    fn test_rmp_roundtrip() {
        let h_orig = AgentPubKey::from_raw_bytes(vec![0xdb; 36]);
        let buf = holochain_serialized_bytes::encode(&h_orig).unwrap();
        let h: AgentPubKey = holochain_serialized_bytes::decode(&buf).unwrap();

        assert_eq!(h_orig, h);
        assert_eq!(*h.hash_type(), hash_type::Agent::new());
    }

    #[test]
    fn test_composite_hashtype_roundtrips() {
        {
            let h_orig =
                AnyDhtHash::from_raw_bytes_and_type(vec![0xdb; 36], hash_type::AnyDht::Header);
            let buf = holochain_serialized_bytes::encode(&h_orig).unwrap();
            let h: AnyDhtHash = holochain_serialized_bytes::decode(&buf).unwrap();
            assert_eq!(h_orig, h);
            assert_eq!(*h.hash_type(), hash_type::AnyDht::Header);
        }
        {
            let h_orig =
                AnyDhtHash::from_raw_bytes_and_type(vec![0xdb; 36], hash_type::AnyDht::Entry);
            let buf = holochain_serialized_bytes::encode(&h_orig).unwrap();
            let h: AnyDhtHash = holochain_serialized_bytes::decode(&buf).unwrap();
            assert_eq!(h_orig, h);
            assert_eq!(*h.hash_type(), hash_type::AnyDht::Entry);
        }
        {
            let h_orig =
                AnyDhtHash::from_raw_bytes_and_type(vec![0xdb; 36], hash_type::AnyDht::Entry);
            let buf = holochain_serialized_bytes::encode(&h_orig).unwrap();
            let h: AnyDhtHash = holochain_serialized_bytes::decode(&buf).unwrap();
            assert_eq!(h_orig, h);
            assert_eq!(*h.hash_type(), hash_type::AnyDht::Entry);
        }
    }

    #[test]
    #[should_panic]
    fn test_composite_hashtype_crossover_error_1() {
        {
            let h_orig = EntryHash::from_raw_bytes_and_type(vec![0xdb; 36], hash_type::Entry);
            let buf = holochain_serialized_bytes::encode(&h_orig).unwrap();
            let _: AnyDhtHash = holochain_serialized_bytes::decode(&buf).unwrap();
        }
    }

    #[test]
    #[should_panic]
    fn test_composite_hashtype_crossover_error_2() {
        {
            let h_orig = EntryHash::from_raw_bytes_and_type(vec![0xdb; 36], hash_type::Entry);
            let buf = holochain_serialized_bytes::encode(&h_orig).unwrap();
            let _: AnyDhtHash = holochain_serialized_bytes::decode(&buf).unwrap();
        }
    }

    #[test]
    #[should_panic]
    fn test_composite_hashtype_crossover_error_3() {
        {
            let h_orig =
                AnyDhtHash::from_raw_bytes_and_type(vec![0xdb; 36], hash_type::AnyDht::Entry);
            let buf = holochain_serialized_bytes::encode(&h_orig).unwrap();
            let _: EntryHash = holochain_serialized_bytes::decode(&buf).unwrap();
        }
    }

    #[test]
    #[should_panic]
    fn test_composite_hashtype_crossover_error_4() {
        {
            let h_orig =
                AnyDhtHash::from_raw_bytes_and_type(vec![0xdb; 36], hash_type::AnyDht::Entry);
            let buf = holochain_serialized_bytes::encode(&h_orig).unwrap();
            let _: EntryHash = holochain_serialized_bytes::decode(&buf).unwrap();
        }
    }

    #[test]
    #[should_panic]
    fn test_composite_hashtype_crossover_error_5() {
        {
            let h_orig =
                AnyDhtHash::from_raw_bytes_and_type(vec![0xdb; 36], hash_type::AnyDht::Header);
            let buf = holochain_serialized_bytes::encode(&h_orig).unwrap();
            let _: EntryHash = holochain_serialized_bytes::decode(&buf).unwrap();
        }
    }

    #[test]
    fn test_struct_to_struct_roundtrip() {
        #[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
        struct TestData {
            e: EntryHash,
            h: HeaderHash,
        }

        let orig = TestData {
            e: EntryHash::from_raw_bytes_and_type(vec![0xdb; 36], hash_type::Entry),
            h: HeaderHash::from_raw_bytes(vec![0xdb; 36]),
        };

        let sb: SerializedBytes = (&orig).try_into().unwrap();
        let res: TestData = sb.try_into().unwrap();

        assert_eq!(orig, res);
        assert_eq!(*orig.e.hash_type(), hash_type::Entry);
        assert_eq!(*orig.h.hash_type(), hash_type::Header);
    }

    #[test]
    fn test_json_to_rust() {
        #[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
        struct Data {
            any_hash: AnyDhtHash,
            content: String,
        }

        let any_hash = AnyDhtHash::from_raw_bytes_and_type(
            b"000000000000000000000000000000000000".to_vec(),
            hash_type::AnyDht::Header,
        );
        let hash_type_sb: SerializedBytes = any_hash.hash_type().try_into().unwrap();
        let hash_type_json = r#"{"Header":[132,41,36]}"#;
        assert_eq!(format!("{:?}", hash_type_sb), hash_type_json.to_string());

        let hash_type_from_sb: hash_type::AnyDht = hash_type_sb.try_into().unwrap();
        assert_eq!(hash_type_from_sb, hash_type::AnyDht::Header);

        let hash_type_from_json: hash_type::AnyDht = serde_json::from_str(&hash_type_json).unwrap();
        assert_eq!(hash_type_from_json, hash_type::AnyDht::Header);
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
                holochain_serialized_bytes::decode(&self.bytes).unwrap()
            }

            fn put(&mut self, k: &K) {
                self.bytes = holochain_serialized_bytes::encode(k).unwrap();
            }
        }

        let mut g: Generic<HeaderHash> = Generic::new();
        let h = HeaderHash::from_raw_bytes(vec![0xdb; 36]);
        g.put(&h);
        assert_eq!(h, g.get());
    }
}
