//! Defines the prefixes for the various HashTypes, as well as the traits
//! which unify them

mod composite;
mod primitive;
pub use composite::*;
pub use primitive::*;

const AGENT_PREFIX: &[u8] = &[0x84, 0x20, 0x24]; // uhCAk
const CONTENT_PREFIX: &[u8] = &[0x84, 0x21, 0x24]; // uhCEk
const DHTOP_PREFIX: &[u8] = &[0x84, 0x24, 0x24]; // uhCQk
const DNA_PREFIX: &[u8] = &[0x84, 0x2d, 0x24]; // uhC0k
const NET_ID_PREFIX: &[u8] = &[0x84, 0x22, 0x24]; // uhCIk
const HEADER_PREFIX: &[u8] = &[0x84, 0x29, 0x24]; // uhCkk
const WASM_PREFIX: &[u8] = &[0x84, 0x2a, 0x24]; // uhCok

/// Every HoloHash is generic over HashType.
/// Additionally, every HashableContent has an associated HashType.
/// The HashType is the glue that binds together HashableContent with its hash.
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
    /// Get the 3 byte prefix for the underlying primitive hash type
    fn get_prefix(self) -> &'static [u8];

    /// Get a Display-worthy name for this hash type
    fn hash_name(self) -> &'static str;
}
