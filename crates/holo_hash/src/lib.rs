mod ext;
pub mod fixt;
mod hashed;
pub use ext::*;
pub use hashed::*;

mod tests;

pub mod prelude {
    pub use super::*;
    pub use holo_hash_core::HasHash;
}

pub use holo_hash_core::HoloHashImpl;
pub type HoloHash<C> = HoloHashImpl<<C as HashableContent>::HashType>;

// re-export hash types
pub use holo_hash_core::{
    AgentPubKey, AnyDhtHash, DhtOpHash, DnaHash, EntryContentHash, EntryHash, HasHash,
    HashableContent, HeaderAddress, HeaderHash, NetIdHash, WasmHash,
};
