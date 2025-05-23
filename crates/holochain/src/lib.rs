//! All the components you need to build a Holochain Conductor

#![recursion_limit = "256"]
#![deny(missing_docs)]

#[cfg(doc)]
pub mod docs;

#[cfg(feature = "hdk")]
pub use hdk::HDI_VERSION;

#[cfg(feature = "hdk")]
pub use hdk::HDK_VERSION;

/// Current Holochain Conductor rust crate version.
pub const HOLOCHAIN_VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod conductor;
pub mod core;
#[cfg(feature = "test_utils")]
pub mod fixt;

#[cfg(any(test, feature = "test_utils"))]
pub mod sweettest;
#[cfg(any(test, feature = "test_utils"))]
pub mod test_utils;

// this is here so that wasm ribosome macros can reference it
pub use holochain_wasmer_host;
pub use tracing;

// TODO can probably move these to integration test once
// we work out the test utils stuff
#[cfg(test)]
mod local_network_tests;

/// Common imports when using the Holochain crate.
pub mod prelude {
    pub use holo_hash;

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
