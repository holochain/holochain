//! Defines the serialization rules for HoloHashes

use std::fmt::Debug;

use crate::HashType;
use crate::HoloHash;
use holochain_serialized_bytes::SerializedBytes;
use holochain_serialized_bytes::SerializedBytesError;
use holochain_serialized_bytes::UnsafeBytes;
use serde::ser::SerializeSeq;

/// Ways of serializing a HoloHash
pub trait HashSerializer: Clone + Debug {
    fn get() -> HashSerialization;
}

/// Ways of serializing a HoloHash
pub enum HashSerialization {
    ByteArray,
    Base64,
    ByteSequence,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
/// This hash is serialized as a byte array
pub struct ByteArraySerializer;

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
/// This hash is serialized as a base64 string
pub struct Base64Serializer;

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
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

impl<T, R> HoloHash<T, R>
where
    T: HashType,
    R: HashSerializer,
{
    /// Change the serialization phantom type
    pub(crate) fn change_serialization<S: HashSerializer>(self) -> HoloHash<T, S> {
        HoloHash {
            hash: self.hash,
            hash_type: self.hash_type,
            _serializer: std::marker::PhantomData,
        }
    }
}

impl<T: HashType, H: HashSerializer> serde::Serialize for HoloHash<T, H> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match H::get() {
            HashSerialization::ByteArray => serializer.serialize_bytes(self.get_raw_39()),
            HashSerialization::Base64 => serializer.serialize_str(&self.to_string()),
            HashSerialization::ByteSequence => {
                let bytes = self.get_raw_39();
                let mut seq = serializer.serialize_seq(Some(bytes.len()))?;
                for element in bytes {
                    seq.serialize_element(element)?;
                }
                seq.end()
            }
        }
    }
}

impl<'de, T: HashType, S: HashSerializer> serde::Deserialize<'de> for HoloHash<T, S> {
    fn deserialize<D>(deserializer: D) -> Result<HoloHash<T, S>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_bytes(HoloHashVisitor(
            std::marker::PhantomData,
            std::marker::PhantomData,
        ))
    }
}

struct HoloHashVisitor<T: HashType, HS: HashSerializer>(
    std::marker::PhantomData<T>,
    std::marker::PhantomData<HS>,
);

impl<'de, T: HashType, HS: HashSerializer> serde::de::Visitor<'de> for HoloHashVisitor<T, HS> {
    type Value = HoloHash<T, HS>;

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

impl<T: HashType, S: HashSerializer> std::convert::TryFrom<&HoloHash<T, S>> for SerializedBytes {
    type Error = SerializedBytesError;
    fn try_from(t: &HoloHash<T, S>) -> std::result::Result<SerializedBytes, SerializedBytesError> {
        match holochain_serialized_bytes::encode(t) {
            Ok(v) => Ok(SerializedBytes::from(UnsafeBytes::from(v))),
            Err(e) => Err(SerializedBytesError::Serialize(e.to_string())),
        }
    }
}

impl<T: HashType, S: HashSerializer> std::convert::TryFrom<HoloHash<T, S>> for SerializedBytes {
    type Error = SerializedBytesError;
    fn try_from(t: HoloHash<T, S>) -> std::result::Result<SerializedBytes, SerializedBytesError> {
        SerializedBytes::try_from(&t)
    }
}

impl<T: HashType, S: HashSerializer> std::convert::TryFrom<SerializedBytes> for HoloHash<T, S> {
    type Error = SerializedBytesError;
    fn try_from(sb: SerializedBytes) -> std::result::Result<HoloHash<T, S>, SerializedBytesError> {
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
    fn test_json_roundtrip() {
        let h_orig = AgentPubKey::from_raw_36(vec![0xdb; HOLO_HASH_UNTYPED_LEN]);
        let json = serde_json::to_string(&h_orig).unwrap();
        let h: AgentPubKey = serde_json::from_str(&json).unwrap();

        assert_eq!(h_orig, h);
        assert_eq!(*h.hash_type(), hash_type::Agent::new());

        // Make sure that the representation is a raw 39-byte array
        let array: TestByteArray = serde_json::from_str(&json).unwrap();
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

        let hash_type_from_json: hash_type::AnyDht = serde_json::from_str(hash_type_json).unwrap();
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
