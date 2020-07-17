const AGENT_PREFIX: &[u8] = &[0x84, 0x20, 0x24]; // uhCAk
const CONTENT_PREFIX: &[u8] = &[0x84, 0x21, 0x24]; // uhCEk
const DHTOP_PREFIX: &[u8] = &[0x84, 0x24, 0x24]; // uhCQk
const DNA_PREFIX: &[u8] = &[0x84, 0x2d, 0x24]; // uhC0k
const NET_ID_PREFIX: &[u8] = &[0x84, 0x22, 0x24]; // uhCIk
const HEADER_PREFIX: &[u8] = &[0x84, 0x29, 0x24]; // uhCkk
const WASM_PREFIX: &[u8] = &[0x84, 0x2a, 0x24]; // uhCok

pub trait HashType:
    Copy
    + Clone
    + std::fmt::Debug
    + Clone
    + std::hash::Hash
    + PartialEq
    + Eq
    + PartialOrd
    + Ord
    + serde::de::DeserializeOwned
    + serde::Serialize
{
    fn get_prefix(self) -> &'static [u8];
    fn hash_name(self) -> &'static str;
}

pub trait PrimitiveHashType: HashType {
    fn new() -> Self;
    fn static_prefix() -> &'static [u8];
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
                    _ => panic!("noo"),
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

#[derive(
    Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize, serde::Serialize,
)]
pub enum Entry {
    Agent,
    Content,
}

impl HashType for Entry {
    fn get_prefix(self) -> &'static [u8] {
        match self {
            Entry::Agent => Agent::new().get_prefix(),
            Entry::Content => Content::new().get_prefix(),
        }
    }
    fn hash_name(self) -> &'static str {
        "EntryHash"
    }
}

#[derive(
    Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize, serde::Serialize,
)]
pub enum AnyDht {
    Entry(Entry),
    Header,
}

impl HashType for AnyDht {
    fn get_prefix(self) -> &'static [u8] {
        match self {
            AnyDht::Entry(entry_type) => entry_type.get_prefix(),
            AnyDht::Header => Header::new().get_prefix(),
        }
    }
    fn hash_name(self) -> &'static str {
        "AnyDhtHash"
    }
}
