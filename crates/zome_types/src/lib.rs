pub mod capability;
pub mod commit;
pub mod crdt;
pub mod debug;
pub mod entry_def;
pub mod entry;
pub mod globals;
pub mod hash;
pub mod header;
pub mod init;
pub mod migrate_agent;
pub mod post_commit;
pub mod validate;
pub mod zome;
mod zome_io;

pub use entry::Entry;
use holochain_serialized_bytes::prelude::*;
pub use zome_io::*;

pub trait CallbackResult {
    /// if a callback result is definitive we should halt any further iterations over remaining
    /// calls e.g. over sparse names or subsequent zomes
    /// typically a clear failure is definitive but success and missing dependencies are not
    /// in the case of success or missing deps, a subsequent callback could give us a definitive
    /// answer like a fail, and we don't want to over-optimise wasm calls and miss a clear failure
    fn is_definitive(&self) -> bool;
}
