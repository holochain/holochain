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
//! As of this writing, we are using rusqlite 0.32.1. You can find the sqlcipher
//! version used here: <https://github.com/rusqlite/rusqlite/blob/v0.32.1/libsqlite3-sys/upgrade_sqlcipher.sh#L11> -- `4.5.7`.
//!
//! #### Building `sqlcipher`
//!
//! Download the source from here: <https://github.com/sqlcipher/sqlcipher/releases/tag/v4.5.7>
//!
//! Unpack and run the build commands per the README.md:
//!
//! ```sh
//! ./configure --enable-tempstore=yes CFLAGS="-DSQLITE_HAS_CODEC" LDFLAGS="-lcrypto"
//! make
//! ```
//!
//! Now you have a compatible sqlcipher cli utility: `./sqlcipher`, but we
//! need the secrets used to encrypt the database.
//!
//! #### Getting the database secrets out of holochain.
//!
//! Holochain stores secrets in a file named `db.key` in the configured
//! `data_root_path`. If you print out the file, it will just be base64:
//!
//! ```sh
//! $ cat /tmp/bob/databases/db.key
//! RXfUEZzCURLrG8hJVcUP4A6T1qY_gql0Fata5PxEgbV7P5IuKoeTu8hyCo9MYdH3vZTU8Loprip22YmRk0vdd_Lcuz3lfKx5FeB_0pskegI_6Zsb4zcTZA
//! ```
//!
//! To decrypt this, we will need the passphrase. We can use a cli flag
//! on holochain, `--danger-print-db-secrets`, which will print the secrets
//! out on stderr:
//!
//! ```sh
//! $ holochain --danger-print-db-secrets -c ~/conductor-config.yaml
//! Initialising log output formatting with option Log
//! # passphrase>
//! # lair-keystore connection_url # unix:///tmp/bob/ks/socket?k=aq19xrSyPaDZbL-Keb8WHhaZ2xbxN07yYztfwqpNAxs #
//! # lair-keystore running #
//! --beg-db-secrets--
//! PRAGMA key = "x'6D71B0A31666195576242A41129FE9387ECA216DA241C98F92A18A01557A8199'";
//! PRAGMA cipher_salt = "x'15E07FD29B247A023FE99B1BE3371364'";
//! PRAGMA cipher_compatibility = 4;
//! PRAGMA cipher_plaintext_header_size = 32;
//! --end-db-secrets--
//!
//! ###HOLOCHAIN_SETUP###
//! ###HOLOCHAIN_SETUP_END###
//! Conductor ready.
//! ```
//!
//! Note the `PRAGMA` directives printed out between the `--beg-db-secrets--`
//! and `--end-db-secrets--` markers.
//!
//! #### Connect to your encrypted holochain database via sqlcipher
//!
//! ```sh
//! ./sqlcipher /tmp/bob/databases/conductor/conductor
//! ```
//!
//! At the `sqlite>` prompt, input your key:
//!
//! ```text
//! PRAGMA key = "x'6D71B0A31666195576242A41129FE9387ECA216DA241C98F92A18A01557A8199'";
//! PRAGMA cipher_salt = "x'15E07FD29B247A023FE99B1BE3371364'";
//! PRAGMA cipher_compatibility = 4;
//! PRAGMA cipher_plaintext_header_size = 32;
//! ```
//!
//! It should print out `ok` for the `key` pragma, and nothing for the other
//! three lines.
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
