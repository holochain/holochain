use super::*;

/// A PrimitiveHashType is one with a multihash prefix.
/// In contrast, a non-primitive hash type could be one of several primitive
/// types, e.g. an `AnyDhtHash` can represent one of three primitive types.
pub trait PrimitiveHashType: HashType {
    /// Constructor
    fn new() -> Self;

    /// Get the 3 byte prefix, which is statically known for primitive hash types
    fn static_prefix() -> &'static [u8];

    /// Get a Display-worthy name for this hash type
    fn hash_name(self) -> &'static str;
}

impl<P: PrimitiveHashType> HashType for P {
    fn get_prefix(self) -> &'static [u8] {
        P::static_prefix()
    }
    fn hash_name(self) -> &'static str {
        PrimitiveHashType::hash_name(self)
    }
}

macro_rules! primitive_hash_type {
    ($name: ident, $display: ident, $visitor: ident, $prefix: ident) => {
        /// The $name PrimitiveHashType
        #[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name;

        impl PrimitiveHashType for $name {
            fn new() -> Self {
                Self
            }
            fn static_prefix() -> &'static [u8] {
                &$prefix
            }
            fn hash_name(self) -> &'static str {
                stringify!($display)
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_bytes(self.get_prefix())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<$name, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                deserializer.deserialize_bytes($visitor)
            }
        }

        struct $visitor;

        impl<'de> serde::de::Visitor<'de> for $visitor {
            type Value = $name;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a HoloHash of primitive hash_type")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match v {
                    $prefix => Ok($name),
                    _ => panic!("unknown hash prefix during hash deserialization {:?}", v),
                }
            }
        }
    };
}

primitive_hash_type!(Agent, AgentPubKey, AgentVisitor, AGENT_PREFIX);
primitive_hash_type!(Content, EntryContentHash, ContentVisitor, CONTENT_PREFIX);
primitive_hash_type!(Dna, DnaHash, DnaVisitor, DNA_PREFIX);
primitive_hash_type!(DhtOp, DhtOpHash, DhtOpVisitor, DHTOP_PREFIX);
primitive_hash_type!(Header, HeaderHash, HeaderVisitor, HEADER_PREFIX);
primitive_hash_type!(NetId, NetIdHash, NetIdVisitor, NET_ID_PREFIX);
primitive_hash_type!(Wasm, WasmHash, WasmVisitor, WASM_PREFIX);
