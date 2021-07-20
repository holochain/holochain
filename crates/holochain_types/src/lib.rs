//! Common types used by other Holochain crates.
//!
//! This crate is a complement to the
//! [holochain_zome_types crate](https://crates.io/crates/holochain_zome_types),
//! which contains only the essential types which are used in Holochain DNA
//! code. This crate expands on those types to include all types which Holochain
//! itself depends on.

#![deny(missing_docs)]
// Toggle this to see what needs to be eventually refactored (as warnings).
#![allow(deprecated)]
// We have a lot of usages of type aliases to `&String`, which clippy objects to.
#![allow(clippy::ptr_arg)]

pub mod access;
pub mod activity;
pub mod app;
pub mod autonomic;
pub mod chain;
pub mod combinators;
pub mod db;
pub mod dht_op;
pub mod dna;
pub mod element;
pub mod entry;
pub mod env;
pub mod fixt;
pub mod header;
pub mod link;
mod macros;
pub mod metadata;
pub mod prelude;
pub mod properties;
pub mod signal;
pub mod timestamp;
pub mod validate;

pub mod test_utils;

pub use holochain_zome_types::entry::EntryHashed;
pub use timestamp::{Timestamp, TimestampKey};
