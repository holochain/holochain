//! The Holochain state crate provides helpers and abstractions for working
//! with the `holochain_sqlite` crate.
//!
//! ## Reads
//! The [`DhtStore`] and [`DhtStoreRead`] types are the main abstractions for
//! reading data, combining database access with the in-memory scratch space.
//!
//! The [`source_chain`] module provides the [`SourceChain`](crate::source_chain::SourceChain) type,
//! which is the abstraction for working with chains of actions.
//!
//! The [`host_fn_workspace`] module provides abstractions for reading data during workflows.
//!
//! ## Writes
//! The [`mutations`] module is the complete set of functions
//! for writing data to sqlite in holochain.
//!
//! ## In-memory
//! The [`scratch`] module provides the [`Scratch`](crate::scratch::Scratch) type for
//! reading and writing data in memory that is not visible anywhere else.
//!
//! The SourceChain type uses the Scratch for in-memory operations which
//! can be flushed to the database.

pub use dht_store::{DhtStore, DhtStoreRead};

/// Re-exports from the `holochain_data` crate.
pub mod data {
    pub use holochain_data::{
        conductor::AppInterfaceModel, kind::*, open_db, DatabaseIdentifier, DbKey, DbSyncLevel,
        HolochainDataConfig, DbRead, DbWrite
    };

    #[cfg(feature = "test_utils")]
    pub use holochain_data::test_open_db;
}

pub mod block;
pub mod chain_lock;
pub mod conductor;
pub mod dht_store;
#[allow(missing_docs)]
pub mod dna_def;
pub mod entry_def;
pub mod host_fn_workspace;
pub mod mutations;
pub mod peer_metadata_store;
#[allow(missing_docs)]
pub mod prelude;
pub mod query;
pub mod schedule;
pub mod scratch;
#[allow(missing_docs)]
pub mod source_chain;
pub mod validation_db;
#[allow(missing_docs)]
pub mod wasm;

#[allow(missing_docs)]
#[cfg(any(test, feature = "test_utils"))]
pub mod test_utils;
