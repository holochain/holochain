//! All the components you need to build a Holochain Conductor

// TODO investigate this lint
#![allow(clippy::result_large_err)]
// We have a lot of usages of type aliases to `&String`, which clippy objects to.
#![allow(clippy::ptr_arg)]
#![recursion_limit = "256"]

#[cfg(doc)]
pub mod docs;

#[cfg(feature = "hdk")]
pub use hdk::HDI_VERSION;

#[cfg(feature = "hdk")]
pub use hdk::HDK_VERSION;

/// Current Holochain Conductor rust crate version.
pub const HOLOCHAIN_VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod conductor;
#[allow(missing_docs)]
pub mod core;
#[allow(missing_docs)]
#[cfg(feature = "test_utils")]
pub mod fixt;

#[cfg(any(test, feature = "test_utils"))]
#[deny(missing_docs)]
pub mod sweettest;
#[cfg(any(test, feature = "test_utils"))]
#[deny(missing_docs)]
pub mod test_utils;

// this is here so that wasm ribosome macros can reference it
pub use holochain_wasmer_host;
pub use tracing;

// TODO can probably move these to integration test once
// we work out the test utils stuff
#[cfg(test)]
mod local_network_tests;

pub mod prelude {
    pub use holo_hash;
    pub use holochain_p2p::{AgentPubKeyExt, DhtOpHashExt, DnaHashExt, HolochainP2pSender};

    #[cfg(feature = "hdk")]
    pub use hdk::link::GetLinksInputBuilder;

    pub use holochain_types::prelude::{fixt, *};

    #[cfg(feature = "fuzzing")]
    pub use kitsune_p2p::{NOISE, *};

    #[cfg(feature = "test_utils")]
    pub use holochain_types::inline_zome::*;
}


#[cfg(all(feature = "wasmer_sys", feature = "wasmer_wamr"))]
compile_error!(
    "feature \"wasmer_sys\" and feature \"wasmer_wamr\" cannot be enabled at the same time"
);

#[cfg(all(not(feature = "wasmer_sys"), not(feature = "wasmer_wamr"),))]
compile_error!("One of: `wasmer_sys`, `wasmer_wamr` features must be enabled. Please, pick one.");

// Temporarily include a fork of wasmer from the git branch 'wamr', until it is officially released in wasmer v5
#[cfg(feature = "wasmer_wamr")]
extern crate hc_wasmer as wasmer;