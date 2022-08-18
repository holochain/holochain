//! The holochain state crate provides helpers and abstractions for working
//! with the `holochain_sqlite` crate.
//!
//! ## Reads
//! The main abstraction for creating data read queries is the [`Query`](query::Query) trait.
//! This can be implemented to make constructing complex queries easier.
//!
//! The [`source_chain`] module provides the [`SourceChain`](source_chain::SourceChain) type,
//! which is the abstraction for working with chains of actions.
//!
//! The [`host_fn_workspace`] module provides abstractions for reading data during workflows.
//!
//! ## Writes
//! The [`mutations`] module is the complete set of functions
//! for writing data to sqlite in holochain.
//!
//! ## In Memory
//! The [`scratch`] module provides the [`Scratch`](scratch::Scratch) type for
//! reading and writing data in memory that is not visible anywhere else.
//!
//! The SourceChain type uses the Scratch for in memory operations which
//! can be flushed to the database.
//!
//! The Query trait allows combining arbitrary database sql queries with
//! the scratch space so reads can union across the database and in memory data.

#![allow(deprecated)]

pub mod chain_lock;
#[allow(missing_docs)]
pub mod dna_def;
pub mod entry_def;
pub mod host_fn_workspace;
pub mod integrate;
pub mod mutations;
#[allow(missing_docs)]
pub mod prelude;
pub mod query;
pub mod schedule;
pub mod scratch;
#[allow(missing_docs)]
pub mod source_chain;
pub mod validation_db;
pub mod validation_receipts;
#[allow(missing_docs)]
pub mod wasm;
pub mod workspace;

#[allow(missing_docs)]
#[cfg(any(test, feature = "test_utils"))]
pub mod test_utils;
