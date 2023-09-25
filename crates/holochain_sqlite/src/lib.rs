//! # Building blocks for persisted Holochain state
//!
//! See crate README for more info.
//!
//! See [this hackmd](https://holo.hackmd.io/@holochain/SkuVLpqEL) for a diagram explaining the relationships between these building blocks and the higher abstractions

pub mod db;
pub mod error;
pub mod exports;
pub mod fatal;
pub mod functions;
#[cfg(not(loom))]
pub mod nonce;
pub mod prelude;
pub mod schema;
#[cfg(not(loom))]
pub mod sql;
pub mod stats;
#[cfg(not(loom))]
pub mod store;
pub mod swansong;

mod table;

// Re-export rusqlite for use with `impl_to_sql_via_as_ref!` macro
pub use ::rusqlite;
