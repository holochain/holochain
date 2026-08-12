//! Holochain Zome Types: only the types needed by Holochain application
//! developers to use in their Zome code, and nothing more.
//!
//! This crate is intentionally kept as minimal as possible, since it is
//! typically included as a dependency in Holochain Zomes, which are
//! distributed as chunks of Wasm. In contrast, the
//! [holochain_types crate](https://crates.io/crates/holochain_types)
//! contains more types which are used by Holochain itself.

#![deny(missing_docs)]
#![allow(non_local_definitions)]

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
pub mod clone;
pub mod dna_def;
pub mod entry;
#[allow(missing_docs)]
pub mod entry_def;
#[allow(missing_docs)]
pub mod info;
#[allow(missing_docs)]
pub mod init;
pub mod judged;
#[allow(missing_docs)]
pub mod link;
pub mod metadata;
pub mod op;
pub mod prelude;
#[cfg(feature = "properties")]
pub mod properties;
pub mod query;
pub mod request;
/// Schedule functions to run outside a direct zome call.
pub mod schedule;
pub mod signal;
pub mod signature;
pub use holochain_timestamp as timestamp;
#[allow(missing_docs)]
pub mod validate;
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

/// Exports every TypeScript declaration this crate contributes to the
/// conductor API bindings: this crate's `ts_alias!` markers, the zome call
/// return types [`metadata::Details`] and [`link::Link`] (unreachable from
/// any conductor request/response — a zome call carries them as an opaque
/// `ExternIO` the client decodes itself), and upstream crates' declarations.
#[cfg(feature = "ts_rs")]
pub fn export_ts_bindings(cfg: &ts_rs::Config) -> Result<(), ts_rs::ExportError> {
    use ts_rs::TS;

    holochain_integrity_types::export_ts_bindings(cfg)?;
    holochain_nonce::export_ts_bindings(cfg)?;
    crate::action::SignedActionTs::export_all(cfg)?;
    crate::warrant::ActionHashAndSigTs::export_all(cfg)?;
    crate::zome::IntegrityZomeTs::export_all(cfg)?;
    crate::zome::CoordinatorZomeTs::export_all(cfg)?;
    crate::metadata::Details::export_all(cfg)?;
    crate::link::Link::export_all(cfg)?;
    Ok(())
}
