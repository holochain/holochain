//! Everything associated with constructing hashes from content:
//! - The HoloHashed type which pairs content with hash
//! - Extension traits for actually constructing HoloHashes from content

#![deny(missing_docs)]

mod ext;
pub mod fixt;
mod hashed;
pub use ext::*;
pub use hashed::*;
mod tests;

/// Common exports
pub mod prelude {
    pub use super::*;
    pub use holo_hash_core::HasHash;
}

pub use holo_hash_core::HoloHashImpl;

/// A convenience type, for specifying a hash by HashableContent rather than
/// by its HashType
pub type HoloHashOf<C> = HoloHashImpl<<C as HashableContent>::HashType>;

// re-export hash types
pub use holo_hash_core::{
    AgentPubKey, AnyDhtHash, DhtOpHash, DnaHash, EntryContentHash, EntryHash, HasHash,
    HashableContent, HeaderAddress, HeaderHash, NetIdHash, WasmHash,
};
