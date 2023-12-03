//! Holochain Integrity Types: only the types needed by Holochain application
//! developers to use in their integrity Zome code, and nothing more.
//!
//! This crate is intentionally kept as minimal as possible, since it is
//! typically included as a dependency in Holochain Zomes, which are
//! distributed as chunks of Wasm.
//!
//! This crate is also designed to be deterministic and more stable than
//! the higher level crates.

#![deny(missing_docs)]

#[allow(missing_docs)]
pub mod action;
pub mod capability;
pub mod chain;
pub mod countersigning;
pub mod dna_properties;
pub mod entry;
#[allow(missing_docs)]
pub mod entry_def;
pub mod genesis;
#[allow(missing_docs)]
pub mod hash;
pub mod info;
#[allow(missing_docs)]
pub mod link;
pub mod op;
pub mod prelude;
pub mod rate_limit;
pub mod record;
pub mod signature;
pub use kitsune_p2p_timestamp as timestamp;
#[allow(missing_docs)]
pub mod validate;
#[allow(missing_docs)]
pub mod x_salsa20_poly1305;
pub mod zome;
#[allow(missing_docs)]
pub mod zome_io;

pub mod trace;

pub use action::Action;
pub use entry::Entry;
pub use prelude::*;

/// Re-exported dependencies
pub mod dependencies {
    pub use ::subtle;
}
