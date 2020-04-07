//! An Entry is a unit of data in a Holochain Source Chain.

pub(crate) mod cap_entries;
pub(crate) mod deletion_entry;
mod entry;
pub mod entry_type;

pub use entry::*;
