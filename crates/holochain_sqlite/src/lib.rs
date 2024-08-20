//! # Building blocks for persisted Holochain state
//!
//! See crate README for more info.
//!
//! See [this hackmd](https://holo.hackmd.io/@holochain/SkuVLpqEL) for a diagram explaining the relationships between these building blocks and the higher abstractions
//!
//! ### Connecting to Encrypted Databases
//!
//! Ubuntu doesn't ship with the correct version of the sqlcipher utility.
//! We're going to need to build it ourselves.
//!
//! As of this writing, we are using rusqlite 0.31. You can find the sqlcipher
//! version used here: <https://github.com/rusqlite/rusqlite/blob/v0.31.0/libsqlite3-sys/upgrade_sqlcipher.sh#L11> -- `4.5.3`.
//!
//! Download the source from here: <https://github.com/sqlcipher/sqlcipher/releases/tag/v4.5.3>
//!
//! Unpack and run the build commands per the README.md:
//!
//! ```text
//! ./configure --enable-tempstore=yes CFLAGS="-DSQLITE_HAS_CODEC" LDFLAGS="-lcrypto"
//! make
//! ```
//!
//! Now you have a compatible sqlcipher cli utility: `./sqlcipher`.
//!
//! Connect to your encrypted holochain database:
//!
//! ```text
//! ./sqlcipher /tmp/holochain-test-environmentsyQCJLKxtXcDuglEQNVAerzPBUCM/databases/conductor/conductor
//! ```
//!
//! At the `sqlite>` prompt, input your key:
//!
//! ```text
//! PRAGMA key = "x'98483C6EB40B6C31A448C22A66DED3B5E5E8D5119CAC8327B655C8B5C483648101010101010101010101010101010101'";
//! ```
//!
//! It should print out `ok`.
//!
//! You should now be able to make sqlite queries:
//!
//! ```text
//! select count(id) from ConductorState;
//! ```

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
