//! Common holochain types crate.

#[deny(missing_docs)]

/// TODO: remove these 2015 edition artifacts (they're going to require a lot of changes)
#[macro_use]
extern crate serde;
#[macro_use]
extern crate holochain_json_derive;

pub mod agent;
pub mod autonomic;
pub mod cell;
pub mod chain_header;
pub mod db;
pub mod dna;
pub mod entry;
pub mod error;
pub mod link;
pub mod nucleus;
pub mod observability;
pub mod persistence;
pub mod prelude;

/// Placeholders to allow other things to compile
#[allow(missing_docs)]
pub mod shims;

pub mod signature;
pub mod time;
pub mod universal_map;

// #[cfg(test)]
pub mod test_utils;
