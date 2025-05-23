//! Common types used by other Holochain crates.
//!
//! This crate is a complement to the
//! [holochain_zome_types crate](https://crates.io/crates/holochain_zome_types),
//! which contains only the essential types which are used in Holochain DNA
//! code. This crate expands on those types to include all types which Holochain
//! itself depends on.

#![deny(missing_docs)]
// We have a lot of usages of type aliases to `&String`, which clippy objects to.
#![allow(clippy::ptr_arg)]
#![allow(non_local_definitions)]

pub mod access;
pub mod action;
pub mod activity;
pub mod app;
pub mod autonomic;
pub mod chain;
pub mod combinators;
pub mod countersigning;
pub mod db;
pub mod dht_op;
pub mod dna;
pub mod entry;
pub mod link;
mod macros;
pub mod metadata;
pub mod prelude;
pub mod rate_limit;
pub mod record;
pub mod share;
pub mod signal;
#[warn(missing_docs)]
pub mod sql;
pub mod validation_receipt;
pub mod warrant;
pub mod web_app;
pub mod zome_types;

#[cfg(feature = "fixturators")]
pub mod fixt;

#[cfg(feature = "test_utils")]
pub mod inline_zome;
pub mod network;
#[cfg(feature = "test_utils")]
pub mod test_utils;
pub mod websocket;

pub use holochain_zome_types::entry::EntryHashed;
