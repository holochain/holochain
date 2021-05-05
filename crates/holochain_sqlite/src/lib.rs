//! # Building blocks for persisted Holochain state
//!
//! See crate README for more info.
//!
//! See [this hackmd](https://holo.hackmd.io/@holochain/SkuVLpqEL) for a diagram explaining the relationships between these building blocks and the higher abstractions

// pub mod buffer;
pub mod conn;
pub mod db;
pub mod error;
pub mod exports;
pub mod fatal;
// pub mod key;
pub mod prelude;
pub mod schema;
pub mod swansong;
// pub mod transaction;

mod table;

#[cfg(any(test, feature = "test_utils"))]
pub mod test_utils;

// Re-export rusqlite for use with `impl_to_sql_via_as_ref!` macro
pub use ::rusqlite;

#[macro_export]
/// Macro to generate a fresh reader from an DbRead with less boilerplate
macro_rules! fresh_reader {
    ($env: expr, $f: expr) => {{
        let mut conn = $env.conn()?;
        $crate::db::ReadManager::with_reader(&mut conn, $f)
    }};
}
