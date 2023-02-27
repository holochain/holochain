//! Holochain Zome Types: only the types needed by Holochain application
//! developers to use in their Zome code, and nothing more.
//!
//! This crate is intentionally kept as minimal as possible, since it is
//! typically included as a dependency in Holochain Zomes, which are
//! distributed as chunks of Wasm. In contrast, the
//! [holochain_types crate](https://crates.io/crates/holochain_types)
//! contains more types which are used by Holochain itself.

#![deny(missing_docs)]

#[allow(missing_docs)]
pub mod action;
#[allow(missing_docs)]
pub mod agent_activity;
#[allow(missing_docs)]
pub mod block;
pub mod bytes;
#[allow(missing_docs)]
pub mod call;
pub mod capability;
pub mod cell;
#[allow(missing_docs)]
pub mod chain;
pub mod countersigning;
#[allow(missing_docs)]
pub mod crdt;
pub mod dna_def;
pub mod entry;
#[allow(missing_docs)]
pub mod entry_def;
pub mod genesis;
#[allow(missing_docs)]
pub mod hash;
#[allow(missing_docs)]
pub mod info;
#[allow(missing_docs)]
pub mod init;
pub mod judged;
#[allow(missing_docs)]
pub mod link;
pub mod metadata;
#[allow(missing_docs)]
pub mod migrate_agent;
#[allow(missing_docs)]
pub mod op;
pub mod prelude;
#[cfg(feature = "properties")]
pub mod properties;
pub mod query;
pub mod rate_limit;
pub mod record;
pub mod request;
/// Schedule functions to run outside a direct zome call.
pub mod schedule;
pub mod signal;
pub mod signature;
pub use kitsune_p2p_timestamp as timestamp;
pub mod trace;
#[allow(missing_docs)]
pub mod validate;
/// Tracking versions between the WASM host and guests and other interfaces.
///
/// Needed to ensure compatibility as code develops.
pub mod version;
pub mod warrant;
#[allow(missing_docs)]
pub mod x_salsa20_poly1305;
#[allow(missing_docs)]
pub mod zome;
#[allow(missing_docs)]
pub mod zome_io;

#[allow(missing_docs)]
#[cfg(feature = "fixturators")]
pub mod fixt;

#[cfg(feature = "test_utils")]
pub mod test_utils;

pub use action::Action;
pub use entry::Entry;
pub use prelude::*;
/// Re-exported dependencies
pub mod dependencies {
    pub use ::holochain_integrity_types;
    pub use ::subtle;
}
use holochain_wasmer_common::WasmError;

#[allow(missing_docs)]
pub trait CallbackResult: Sized {
    /// if a callback result is definitive we should halt any further iterations over remaining
    /// calls e.g. over sparse names or subsequent zomes
    /// typically a clear failure is definitive but success and missing dependencies are not
    /// in the case of success or missing deps, a subsequent callback could give us a definitive
    /// answer like a fail, and we don't want to over-optimise wasm calls and miss a clear failure
    fn is_definitive(&self) -> bool;
    /// when a WasmError is returned from a callback (e.g. via `?` operator) it might mean either:
    ///
    /// - There was an error that prevented the callback from coming to a CallbackResult (e.g. failing to connect to database)
    /// - There was an error that should be interpreted as a CallbackResult::Fail (e.g. data failed to deserialize)
    ///
    /// Typically this can be split as host/wasm errors are the former, and serialization/guest errors the latter.
    /// This function allows each CallbackResult to explicitly map itself.
    fn try_from_wasm_error(wasm_error: WasmError) -> Result<Self, WasmError>;
}

/// Helper macro for implementing ToSql, when using rusqlite as a dependency
#[macro_export]
macro_rules! impl_to_sql_via_as_ref {
    ($s: ty) => {
        impl ::rusqlite::ToSql for $s {
            fn to_sql(&self) -> ::rusqlite::Result<::rusqlite::types::ToSqlOutput<'_>> {
                Ok(::rusqlite::types::ToSqlOutput::Borrowed(
                    self.as_ref().into(),
                ))
            }
        }
    };
}

/// Helper macro for implementing ToSql, when using rusqlite as a dependency
#[macro_export]
macro_rules! impl_to_sql_via_display {
    ($s: ty) => {
        impl ::rusqlite::ToSql for $s {
            fn to_sql(&self) -> ::rusqlite::Result<::rusqlite::types::ToSqlOutput<'_>> {
                Ok(::rusqlite::types::ToSqlOutput::Owned(
                    self.to_string().into(),
                ))
            }
        }
    };
}

/// 10MB of entropy free for the taking.
/// Useful for initializing arbitrary::Unstructured data
#[cfg(any(test, feature = "test_utils"))]
pub static NOISE: once_cell::sync::Lazy<Vec<u8>> = once_cell::sync::Lazy::new(|| {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    std::iter::repeat_with(|| rng.gen())
        .take(10_000_000)
        .collect()
});
