use super::*;
use crate::error::HoloHashError;
use crate::hash_type;
use crate::AgentPubKey;
use crate::EntryHash;
use std::convert::TryInto;
// Valid Holochain options for prefixes:
// hCAk 4100 <Buffer 84 20 24> * AGENT
// hCEk 4228 <Buffer 84 21 24> * ENTRY
// hCIk 4356 <Buffer 84 22 24> * NET_ID
// hCMk 4484 <Buffer 84 23 24>
// hCQk 4612 <Buffer 84 24 24> * DHTOP
// hCUk 4740 <Buffer 84 25 24>
// hCYk 4868 <Buffer 84 26 24>
// hCck 4996 <Buffer 84 27 24>
// hCgk 5124 <Buffer 84 28 24>
// hCkk 5252 <Buffer 84 29 24> * HEADER
// hCok 5380 <Buffer 84 2a 24> * WASM
// hCsk 5508 <Buffer 84 2b 24>
// hCwk 5636 <Buffer 84 2c 24>
// hC0k 5764 <Buffer 84 2d 24> * DNA
// hC4k 5892 <Buffer 84 2e 24>
// hC8k 6020 <Buffer 84 2f 24> * EXTERNAL

// Valid Holo.Host options for prefixes:
// hhAk 2054 <Buffer 86 10 24> * HOST KEY
// hhEk 2182 <Buffer 86 11 24>
// hhIk 2310 <Buffer 86 12 24>
// hhMk 2438 <Buffer 86 13 24>
// hhQk 2566 <Buffer 86 14 24> * ANONYMOUS KEY (They can Query-only / no source chain)
// hhUk 2694 <Buffer 86 15 24> * WEB USER KEY
// hhYk 2822 <Buffer 86 16 24>
// hhck 2950 <Buffer 86 17 24>
// hhgk 3078 <Buffer 86 18 24>
// hhkk 3206 <Buffer 86 19 24>
// hhok 3334 <Buffer 86 1a 24>
// hhsk 3462 <Buffer 86 1b 24>
// hhwk 3590 <Buffer 86 1c 24>
// hh0k 3718 <Buffer 86 1d 24> * HOSTED APP Bundle (UI with websdk, etc.)
// hh4k 3846 <Buffer 86 1e 24>
// hh8k 3974 <Buffer 86 1f 24>

pub(crate) const AGENT_PREFIX: &[u8] = &[0x84, 0x20, 0x24]; // uhCAk [132, 32, 36]
pub(crate) const ENTRY_PREFIX: &[u8] = &[0x84, 0x21, 0x24]; // uhCEk [132, 33, 36]
pub(crate) const DHTOP_PREFIX: &[u8] = &[0x84, 0x24, 0x24]; // uhCQk [132, 36, 36]
pub(crate) const DNA_PREFIX: &[u8] = &[0x84, 0x2d, 0x24]; // uhC0k [132, 45, 36]
pub(crate) const NET_ID_PREFIX: &[u8] = &[0x84, 0x22, 0x24]; // uhCIk [132, 34, 36]
pub(crate) const HEADER_PREFIX: &[u8] = &[0x84, 0x29, 0x24]; // uhCkk [132, 41, 36]
pub(crate) const WASM_PREFIX: &[u8] = &[0x84, 0x2a, 0x24]; // uhCok [132, 42, 36]
pub(crate) const EXTERNAL_PREFIX: &[u8] = &[0x84, 0x2f, 0x24]; // uhC8k [132, 47, 36]

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

    fn try_from_prefix(prefix: &[u8]) -> HoloHashResult<Self> {
        if prefix == P::static_prefix() {
            Ok(P::new())
        } else {
            Err(HoloHashError::BadPrefix(
                PrimitiveHashType::hash_name(P::new()).to_string(),
                prefix.try_into().expect("3 byte prefix"),
            ))
        }
    }

    fn hash_name(self) -> &'static str {
        PrimitiveHashType::hash_name(self)
    }
}

macro_rules! primitive_hash_type {
    ($name: ident, $display: ident, $visitor: ident, $prefix: ident) => {
        /// The $name PrimitiveHashType
        #[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
        #[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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

        #[cfg(feature = "serialization")]
        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_bytes(self.get_prefix())
            }
        }

        #[cfg(feature = "serialization")]
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<$name, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                deserializer.deserialize_bytes($visitor)
            }
        }

        #[cfg(feature = "serialization")]
        struct $visitor;

        #[cfg(feature = "serialization")]
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
        }
    };
}

primitive_hash_type!(Agent, AgentPubKey, AgentVisitor, AGENT_PREFIX);
primitive_hash_type!(Entry, EntryHash, EntryVisitor, ENTRY_PREFIX);
primitive_hash_type!(Dna, DnaHash, DnaVisitor, DNA_PREFIX);
primitive_hash_type!(DhtOp, DhtOpHash, DhtOpVisitor, DHTOP_PREFIX);
primitive_hash_type!(Header, HeaderHash, HeaderVisitor, HEADER_PREFIX);
primitive_hash_type!(NetId, NetIdHash, NetIdVisitor, NET_ID_PREFIX);
primitive_hash_type!(Wasm, WasmHash, WasmVisitor, WASM_PREFIX);
primitive_hash_type!(External, ExternalHash, ExternalVisitor, EXTERNAL_PREFIX);

// DhtOps are mostly hashes
impl HashTypeSync for DhtOp {}
// Entries are capped at 16MB, which is small enough to hash synchronously
impl HashTypeSync for Entry {}
// Headers are only a few hundred bytes at most
impl HashTypeSync for Header {}
// A DnaHash is a hash of the DnaDef, which excludes the wasm bytecode
impl HashTypeSync for Dna {}

// We don't know what external data might be getting hashed but typically it
// would be small, like a reference to something in an external system such as
// a hash in a different DHT, an IPFS hash or a UUID, etc.
/// External hashes have a DHT location and hash prefix like all other native
/// holochain hashes but are NOT found/fetchable on the DHT.
/// External hashing makes no assumptions about the data that was digested to
/// create the hash so arbitrary bytes can be passed in.
/// It is valid to EITHER use an existing 32 byte hash/data as literal bytes
/// for an external hash (literal+prefix, no data loss) OR digest arbitrary
/// data into an external hash (support all data, opaque result).
impl HashTypeSync for External {}

impl HashTypeAsync for NetId {}
impl HashTypeAsync for Wasm {}

impl From<AgentPubKey> for EntryHash {
    fn from(hash: AgentPubKey) -> EntryHash {
        hash.retype(hash_type::Entry)
    }
}

impl From<EntryHash> for AgentPubKey {
    fn from(hash: EntryHash) -> AgentPubKey {
        hash.retype(hash_type::Agent)
    }
}
