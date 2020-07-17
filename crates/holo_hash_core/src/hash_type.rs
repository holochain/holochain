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
    + serde::Serialize
    + serde::de::DeserializeOwned
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
    ($name: ident, $display: ident, $prefix: ident) => {
        #[derive(
            Debug,
            Copy,
            Clone,
            Hash,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            serde::Serialize,
            serde::Deserialize,
        )]
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
    };
}

primitive_hash_type!(Agent, AgentPubKey, AGENT_PREFIX);
primitive_hash_type!(Content, EntryContentHash, CONTENT_PREFIX);
primitive_hash_type!(Dna, DnaHash, DNA_PREFIX);
primitive_hash_type!(DhtOp, DhtOpHash, DHTOP_PREFIX);
primitive_hash_type!(Header, HeaderHash, HEADER_PREFIX);
primitive_hash_type!(NetId, NetIdHash, NET_ID_PREFIX);
primitive_hash_type!(Wasm, WasmHash, WASM_PREFIX);

#[derive(
    Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
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
    Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
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
