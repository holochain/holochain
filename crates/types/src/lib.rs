//! Common types used by other Holochain crates.
//!
//! This crate is a complement to the [holochain_zome_types crate](https://crates.io/crates/holochain_zome_types), which contains only the essential types which are used in Holochain DNA code. This crate expands on those types to include all types which Holochain itself depends on.

#![deny(missing_docs)]

pub mod app;
pub mod autonomic;
pub mod cell;
pub mod db;
pub mod dht_op;
pub mod dna;
pub mod element;
pub mod entry;
pub mod fixt;
pub mod header;
pub mod link;
pub mod metadata;
pub mod prelude;
pub mod timestamp;
pub mod validate;

/// Placeholders to allow other things to compile
#[allow(missing_docs)]
pub mod shims;

pub mod universal_map;

// #[cfg(test)]
pub mod test_utils;

#[doc(inline)]
pub use entry::{Entry, EntryHashed};

#[doc(inline)]
pub use header::HeaderHashed;

pub use timestamp::{Timestamp, TimestampKey};

pub use observability;
