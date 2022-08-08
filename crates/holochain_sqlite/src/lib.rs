//! # Building blocks for persisted Holochain state
//!
//! See crate README for more info.
//!
//! See [this hackmd](https://holo.hackmd.io/@holochain/SkuVLpqEL) for a diagram explaining the relationships between these building blocks and the higher abstractions

pub mod conn;
pub mod db;
pub mod error;
pub mod exports;
pub mod fatal;
pub mod functions;
pub mod nonce;
pub mod prelude;
pub mod schema;
pub mod sql;
pub mod swansong;

mod table;

#[cfg(any(test, feature = "test_utils"))]
pub mod test_utils;

// Re-export rusqlite for use with `impl_to_sql_via_as_ref!` macro
pub use ::rusqlite;
