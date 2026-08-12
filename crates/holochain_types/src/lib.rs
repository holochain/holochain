//! Common types used by other Holochain crates.
//!
//! This crate is a complement to the
//! [holochain_zome_types crate](https://crates.io/crates/holochain_zome_types),
//! which contains only the essential types which are used in Holochain DNA
//! code. This crate expands on those types to include all types which Holochain
//! itself depends on.

#![deny(missing_docs)]
// We have a lot of usages of type aliases to `&String`, which clippy objects to.
#![allow(clippy::ptr_arg)]

pub mod access;
pub mod activity;
pub mod app;
pub mod cell_config_overrides;
pub mod chain;
pub mod combinators;
pub mod countersigning;
pub mod dna;
pub mod entry;
pub mod error;
pub mod link;
mod macros;
pub mod op;
pub mod prelude;
pub mod record;
pub mod report;
pub mod share;
pub mod signal;
pub mod validation_receipt;
pub mod warrant;
pub mod web_app;
pub mod wire_ops;
pub mod zome_types;

#[cfg(feature = "fixturators")]
pub mod fixt;

#[cfg(feature = "test_utils")]
pub mod inline_zome;
pub mod network;
#[cfg(feature = "test_utils")]
pub mod test_utils;
pub mod websocket;

/// Exports every TypeScript declaration this crate contributes to the
/// conductor API bindings: this crate's `ts_alias!` markers, the
/// countersigning session types (exported unconditionally, independent of
/// `unstable-countersigning`), [`op::DhtOp`] (unreachable from any request
/// or response), and upstream crates' declarations.
///
/// Items gated behind other `unstable-*` features (e.g.
/// `unstable-migration`'s `DnaManifestV0::lineage`) are compiled out of the
/// export build and absent from the bindings.
#[cfg(feature = "ts_rs")]
pub fn export_ts_bindings(cfg: &ts_rs::Config) -> Result<(), ts_rs::ExportError> {
    use ts_rs::TS;

    holochain_zome_types::export_ts_bindings(cfg)?;
    mr_bundle::ts::export_ts_bindings(cfg)?;
    crate::websocket::AllowedOriginsTs::export_all(cfg)?;
    crate::network::BlockedMessageCountsMapTs::export_all(cfg)?;
    crate::app::MemproofMapTs::export_all(cfg)?;
    crate::app::RoleSettingsMapTs::export_all(cfg)?;
    crate::app::EnableCloneCellPayloadTs::export_all(cfg)?;
    crate::countersigning::CountersigningSessionState::export_all(cfg)?;
    crate::op::DhtOp::export_all(cfg)?;
    crate::signal::Signal::export_all(cfg)?;
    crate::app::AppBundle::export_all(cfg)?;
    crate::dna::DnaBundle::export_all(cfg)?;
    crate::prelude::DnaSource::export_all(cfg)?;
    crate::dna::ValidatedDnaManifest::export_all(cfg)?;
    crate::validation_receipt::SignedValidationReceipt::export_all(cfg)?;
    Ok(())
}
