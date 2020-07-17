const AGENT_PREFIX: &[u8] = &[0x84, 0x20, 0x24]; // uhCAk
const CONTENT_PREFIX: &[u8] = &[0x84, 0x21, 0x24]; // uhCEk
const DHTOP_PREFIX: &[u8] = &[0x84, 0x24, 0x24]; // uhCQk
const DNA_PREFIX: &[u8] = &[0x84, 0x2d, 0x24]; // uhC0k
const NET_ID_PREFIX: &[u8] = &[0x84, 0x22, 0x24]; // uhCIk
const HEADER_PREFIX: &[u8] = &[0x84, 0x29, 0x24]; // uhCkk
const WASM_PREFIX: &[u8] = &[0x84, 0x2a, 0x24]; // uhCok

pub enum HashType {
    Primitive(PrimitiveHashType),
    Entry(Entry),
    AnyDht(AnyDht),
}

impl HashTypeT for HashType {
    fn prefix(&self) -> &'static [u8] {
        match self {
            HashType::Primitive(t) => t.prefix(),
            HashType::Entry(t) => t.prefix(),
            HashType::AnyDht(t) => t.prefix(),
        }
    }
}

trait HashTypeT {
    fn prefix(&self) -> &'static [u8];
}

macro_rules! primitive_hash_types {
    ( $( ( $name: ident, $display: ident, $prefix: ident ) ),* $(,)? ) => {

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

        pub enum PrimitiveHashType {
            $( $name ),*
        }

        impl HashTypeT for PrimitiveHashType {
            fn prefix(&self) -> &'static [u8] {
                match self {
                    $( PrimitiveHashType::$name => $prefix ),*
                }
            }
        }

        impl std::fmt::Display for PrimitiveHashType {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $( PrimitiveHashType::$name => write!(
                        f,
                        stringify!($display),
                    ) ),*
                }
            }
        }
    };
}

primitive_hash_types! {
    (Agent, AgentPubKey, AGENT_PREFIX),
    (Content, EntryContentHash, CONTENT_PREFIX),
    (Dna, DnaHash, DNA_PREFIX),
    (DhtOp, DhtOpHash, DHTOP_PREFIX),
    (Header, HeaderHash, HEADER_PREFIX),
    (NetId, NetIdHash, NET_ID_PREFIX),
    (Wasm, WasmHash, WASM_PREFIX),
}

pub enum Entry {
    Agent,
    Content,
}

impl HashTypeT for Entry {
    fn prefix(&self) -> &'static [u8] {
        match self {
            Entry::Agent => PrimitiveHashType::Agent.prefix(),
            Entry::Content => PrimitiveHashType::Content.prefix(),
        }
    }
}

pub enum AnyDht {
    Agent,
    Content,
    Header,
}

impl HashTypeT for AnyDht {
    fn prefix(&self) -> &'static [u8] {
        match self {
            AnyDht::Agent => PrimitiveHashType::Agent.prefix(),
            AnyDht::Content => PrimitiveHashType::Content.prefix(),
            AnyDht::Header => PrimitiveHashType::Header.prefix(),
        }
    }
}
