//! Common holochain types crate.

#![allow(clippy::cognitive_complexity)]
#![deny(missing_docs)]

pub mod autonomic;
pub mod cell;
pub mod composite_hash;
pub mod db;
pub mod dna;
pub mod entry;
pub mod fixt;
pub mod header;
pub mod link;
pub mod nucleus;
pub mod observability;
pub mod prelude;
mod timestamp;
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
pub use header::{Header, HeaderHashed};

pub use timestamp::Timestamp;
