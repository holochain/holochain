//! Holochain Zome Types: only the types needed by Holochain application
//! developers to use in their Zome code, and nothing more.
//!
//! This crate is intentionally kept as minimal as possible, since it is
//! typically included as a dependency in Holochain Zomes, which are
//! distributed as chunks of Wasm. In contrast, the
//! [holochain_types crate](https://crates.io/crates/holochain_types)
//! contains more types which are used by Holochain itself.

#![deny(missing_docs)]

#[allow(missing_docs)]
pub mod agent_info;
pub mod bytes;
#[allow(missing_docs)]
pub mod call;
#[allow(missing_docs)]
pub mod call_remote;
pub mod capability;
pub mod cell;
#[allow(missing_docs)]
pub mod crdt;
pub mod debug;
pub mod element;
pub mod entry;
#[allow(missing_docs)]
pub mod entry_def;
#[allow(missing_docs)]
pub mod header;
#[allow(missing_docs)]
pub mod init;
#[allow(missing_docs)]
pub mod link;
pub mod metadata;
#[allow(missing_docs)]
pub mod migrate_agent;
#[allow(missing_docs)]
pub mod post_commit;
pub mod prelude;
pub mod query;
pub mod request;
pub mod signal;
pub mod signature;
pub mod timestamp;
#[allow(missing_docs)]
pub mod validate;
#[allow(missing_docs)]
pub mod validate_link;
pub mod warrant;
#[allow(missing_docs)]
pub mod zome;
#[allow(missing_docs)]
pub mod zome_info;
#[allow(missing_docs)]
pub mod zome_io;

#[allow(missing_docs)]
#[cfg(feature = "fixturators")]
pub mod fixt;

pub mod test_utils;

pub use agent_info::*;
pub use bytes::*;
pub use call::*;
pub use call_remote::*;
pub use capability::*;
pub use cell::*;
pub use crdt::*;
pub use debug::*;
pub use element::*;
pub use entry::*;
pub use entry_def::*;
pub use header::*;
pub use init::*;
pub use link::*;
pub use metadata::*;
pub use migrate_agent::*;
pub use post_commit::*;
pub use prelude::*;
pub use query::*;
pub use request::*;
pub use signal::*;
pub use signature::*;
pub use timestamp::*;
pub use validate::*;
pub use validate_link::*;
pub use warrant::*;
pub use zome::*;
pub use zome_info::*;
pub use zome_io::*;

#[allow(missing_docs)]
pub trait CallbackResult {
    /// if a callback result is definitive we should halt any further iterations over remaining
    /// calls e.g. over sparse names or subsequent zomes
    /// typically a clear failure is definitive but success and missing dependencies are not
    /// in the case of success or missing deps, a subsequent callback could give us a definitive
    /// answer like a fail, and we don't want to over-optimise wasm calls and miss a clear failure
    fn is_definitive(&self) -> bool;
}
