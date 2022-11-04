//! All the components you need to build a Holochain Conductor

// We have a lot of usages of type aliases to `&String`, which clippy objects to.
#![allow(clippy::ptr_arg)]
#![recursion_limit = "128"]

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
    pub use holochain_p2p::AgentPubKeyExt;
    pub use holochain_p2p::*;
    pub use holochain_types::inline_zome::*;
    pub use holochain_types::prelude::*;
    pub use kitsune_p2p::*;
}
