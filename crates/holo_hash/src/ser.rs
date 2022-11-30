//! Defines the serialization rules for HoloHashes

use crate::HashType;
use crate::HoloHash;
use holochain_serialized_bytes::SerializedBytes;
use holochain_serialized_bytes::SerializedBytesError;
use holochain_serialized_bytes::UnsafeBytes;

impl<T: HashType> serde::Serialize for HoloHash<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(self.get_raw_39())
    }
}

impl<'de, T: HashType> serde::Deserialize<'de> for HoloHash<T> {
    fn deserialize<D>(deserializer: D) -> Result<HoloHash<T>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_bytes(HoloHashVisitor(std::marker::PhantomData))
    }
}

struct HoloHashVisitor<T: HashType>(std::marker::PhantomData<T>);

impl<'de, T: HashType> serde::de::Visitor<'de> for HoloHashVisitor<T> {
    type Value = HoloHash<T>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a HoloHash of primitive hash_type")
    }

    fn visit_bytes<E>(self, h: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if !h.len() == 39 {
            Err(serde::de::Error::custom(
                "HoloHash serialized representation must be exactly 39 bytes",
            ))
        } else {
            HoloHash::from_raw_39(h.to_vec())
                .map_err(|e| serde::de::Error::custom(format!("HoloHash error: {:?}", e)))
        }
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut vec = Vec::with_capacity(seq.size_hint().unwrap_or(0));

        while let Some(b) = seq.next_element()? {
            vec.push(b);
        }

        self.visit_bytes(&vec)
    }

    #[cfg(feature = "encoding")]
    fn visit_str<E>(self, b64: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let h = crate::holo_hash_decode_unchecked(b64)
            .map_err(|e| serde::de::Error::custom(format!("HoloHash error: {:?}", e)))?;
        if !h.len() == 39 {
            Err(serde::de::Error::custom(
                "HoloHash serialized representation must be exactly 39 bytes",
            ))
        } else {
            HoloHash::from_raw_39(h.to_vec())
                .map_err(|e| serde::de::Error::custom(format!("HoloHash error: {:?}", e)))
        }
    }
}

impl<T: HashType> std::convert::TryFrom<&HoloHash<T>> for SerializedBytes {
    type Error = SerializedBytesError;
    fn try_from(t: &HoloHash<T>) -> std::result::Result<SerializedBytes, SerializedBytesError> {
        match holochain_serialized_bytes::encode(t) {
            Ok(v) => Ok(SerializedBytes::from(UnsafeBytes::from(v))),
            Err(e) => Err(SerializedBytesError::Serialize(e.to_string())),
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
            Err(e) => Err(SerializedBytesError::Deserialize(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use holochain_serialized_bytes::prelude::*;
    use std::convert::TryInto;

    #[derive(serde::Deserialize, Debug)]
    #[serde(transparent)]
    struct TestByteArray(#[serde(with = "serde_bytes")] Vec<u8>);

    #[test]
    #[cfg(feature = "serialization")]
    fn test_serialized_bytes_roundtrip() {
        use holochain_serialized_bytes::SerializedBytes;
        use std::convert::TryInto;

        let h_orig = DnaHash::from_raw_36(vec![0xdb; HOLO_HASH_UNTYPED_LEN]);
        let h: SerializedBytes = h_orig.clone().try_into().unwrap();
        let h: DnaHash = h.try_into().unwrap();

        assert_eq!(h_orig, h);
        assert_eq!(*h.hash_type(), hash_type::Dna::new());
    }

    #[test]
    fn test_rmp_roundtrip() {
        let h_orig = AgentPubKey::from_raw_36(vec![0xdb; HOLO_HASH_UNTYPED_LEN]);
        let buf = holochain_serialized_bytes::encode(&h_orig).unwrap();
        let h: AgentPubKey = holochain_serialized_bytes::decode(&buf).unwrap();

        assert_eq!(h_orig, h);
        assert_eq!(*h.hash_type(), hash_type::Agent::new());

        // Make sure that the representation is a raw 39-byte array
        let array: TestByteArray = holochain_serialized_bytes::decode(&buf).unwrap();
        assert_eq!(array.0.len(), HOLO_HASH_FULL_LEN);
        assert_eq!(
            array.0,
            vec![
                132, 32, 36, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219,
                219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219, 219,
                219, 219, 219, 219, 219, 219,
            ]
        );
    }

    #[test]
    fn test_composite_hashtype_roundtrips() {
        {
            let h_orig = AnyDhtHash::from_raw_36_and_type(
                vec![0xdb; HOLO_HASH_UNTYPED_LEN],
                hash_type::AnyDht::Action,
            );
            let buf = holochain_serialized_bytes::encode(&h_orig).unwrap();
            let h: AnyDhtHash = holochain_serialized_bytes::decode(&buf).unwrap();
            assert_eq!(h_orig, h);
            assert_eq!(*h.hash_type(), hash_type::AnyDht::Action);
        }
        {
            let h_orig = AnyDhtHash::from_raw_36_and_type(
                vec![0xdb; HOLO_HASH_UNTYPED_LEN],
                hash_type::AnyDht::Entry,
            );
            let buf = holochain_serialized_bytes::encode(&h_orig).unwrap();
            let h: AnyDhtHash = holochain_serialized_bytes::decode(&buf).unwrap();
            assert_eq!(h_orig, h);
            assert_eq!(*h.hash_type(), hash_type::AnyDht::Entry);
        }
        {
            let h_orig = AnyDhtHash::from_raw_36_and_type(
                vec![0xdb; HOLO_HASH_UNTYPED_LEN],
                hash_type::AnyDht::Entry,
            );
            let buf = holochain_serialized_bytes::encode(&h_orig).unwrap();
            let h: AnyDhtHash = holochain_serialized_bytes::decode(&buf).unwrap();
            assert_eq!(h_orig, h);
            assert_eq!(*h.hash_type(), hash_type::AnyDht::Entry);
        }
    }

    #[test]
    fn test_any_dht_deserialization() {
        {
            let h_orig = EntryHash::from_raw_36_and_type(
                vec![0xdb; HOLO_HASH_UNTYPED_LEN],
                hash_type::Entry,
            );
            let buf = holochain_serialized_bytes::encode(&h_orig).unwrap();
            let _: AnyDhtHash = holochain_serialized_bytes::decode(&buf).unwrap();
        }
        {
            let h_orig = ActionHash::from_raw_36_and_type(
                vec![0xdb; HOLO_HASH_UNTYPED_LEN],
                hash_type::Action,
            );
            let buf = holochain_serialized_bytes::encode(&h_orig).unwrap();
            let _: AnyDhtHash = holochain_serialized_bytes::decode(&buf).unwrap();
        }
    }

    #[test]
    #[should_panic]
    fn test_any_dht_deserialization_crossover_error() {
        {
            let h_orig = DhtOpHash::from_raw_36_and_type(
                vec![0xdb; HOLO_HASH_UNTYPED_LEN],
                hash_type::DhtOp,
            );
            let buf = holochain_serialized_bytes::encode(&h_orig).unwrap();
            let _: AnyDhtHash = holochain_serialized_bytes::decode(&buf).unwrap();
        }
    }

    #[test]
    fn test_struct_to_struct_roundtrip() {
        #[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
        struct TestData {
            e: EntryHash,
            h: ActionHash,
        }

        let orig = TestData {
            e: EntryHash::from_raw_36_and_type(vec![0xdb; HOLO_HASH_UNTYPED_LEN], hash_type::Entry),
            h: ActionHash::from_raw_36(vec![0xdb; HOLO_HASH_UNTYPED_LEN]),
        };

        let sb: SerializedBytes = (&orig).try_into().unwrap();
        let res: TestData = sb.try_into().unwrap();

        assert_eq!(orig, res);
        assert_eq!(*orig.e.hash_type(), hash_type::Entry);
        assert_eq!(*orig.h.hash_type(), hash_type::Action);
    }

    #[test]
    fn test_json_to_rust() {
        #[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
        struct Data {
            any_hash: AnyDhtHash,
            content: String,
        }

        let any_hash = AnyDhtHash::from_raw_36_and_type(
            b"000000000000000000000000000000000000".to_vec(),
            hash_type::AnyDht::Action,
        );
        let hash_type_sb: SerializedBytes = any_hash.hash_type().try_into().unwrap();
        let hash_type_json = r#"{"Action":[132,41,36]}"#;
        assert_eq!(format!("{:?}", hash_type_sb), hash_type_json.to_string());

        let hash_type_from_sb: hash_type::AnyDht = hash_type_sb.try_into().unwrap();
        assert_eq!(hash_type_from_sb, hash_type::AnyDht::Action);

        let hash_type_from_json: hash_type::AnyDht = serde_json::from_str(&hash_type_json).unwrap();
        assert_eq!(hash_type_from_json, hash_type::AnyDht::Action);
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

        let mut g: Generic<ActionHash> = Generic::new();
        let h = ActionHash::from_raw_36(vec![0xdb; HOLO_HASH_UNTYPED_LEN]);
        g.put(&h);
        assert_eq!(h, g.get());
    }
}
