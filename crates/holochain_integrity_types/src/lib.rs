//! Holochain Integrity Types: only the types needed by Holochain application
//! developers to use in their integrity Zome code, and nothing more.
//!
//! This crate is intentionally kept as minimal as possible, since it is
//! typically included as a dependency in Holochain Zomes, which are
//! distributed as chunks of Wasm.
//!
//! This crate is also designed to be deterministic and more stable than
//! the higher level crates.

#![deny(missing_docs)]

#[allow(missing_docs)]
pub mod action;
pub mod capability;
pub mod chain;
pub mod countersigning;
mod dna_modifiers;
pub mod entry;
#[allow(missing_docs)]
pub mod entry_def;
pub mod genesis;
pub mod get_strategy;
pub mod info;
#[allow(missing_docs)]
pub mod link;
pub mod op;
pub mod prelude;
pub mod record;
pub mod signature;
pub use holochain_timestamp as timestamp;
#[allow(missing_docs)]
pub mod validate;
#[allow(missing_docs)]
pub mod x_salsa20_poly1305;
pub mod zome;
#[allow(missing_docs)]
pub mod zome_io;

pub mod trace;

/// Exports every TypeScript declaration this crate contributes to the
/// conductor API bindings: this crate's `ts_alias!` markers and hand-written
/// impls, plus the zome call return types [`record::Record`] and
/// [`op::AgentActivity`] (unreachable from any conductor request/response —
/// a zome call carries them as an opaque `ExternIO` the client decodes
/// itself), and upstream crates' declarations.
#[cfg(feature = "ts_rs")]
pub fn export_ts_bindings(cfg: &ts_rs::Config) -> Result<(), ts_rs::ExportError> {
    use ts_rs::TS;

    holo_hash::ts::export_ts_bindings(cfg)?;
    crate::action::ActionHashedTs::export_all(cfg)?;
    crate::action::SignedActionHashedTs::export_all(cfg)?;
    crate::genesis::MembraneProofTs::export_all(cfg)?;
    crate::countersigning::CounterSigningAgentsTs::export_all(cfg)?;
    crate::info::NetworkSeedTs::export_all(cfg)?;
    crate::prelude::DnaModifiersOpt::<holochain_serialized_bytes::SerializedBytes>::export_all(
        cfg,
    )?;
    crate::capability::GrantedFunctionTs::export_all(cfg)?;
    crate::capability::CapAccess::export_all(cfg)?;
    crate::action::ActionType::export_all(cfg)?;
    crate::countersigning::PreflightResponse::export_all(cfg)?;
    crate::record::Record::export_all(cfg)?;
    crate::op::AgentActivity::export_all(cfg)?;
    Ok(())
}
