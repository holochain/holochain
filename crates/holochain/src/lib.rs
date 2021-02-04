//! All the components you need to build a Holochain Conductor

// #![deny(missing_docs)]
#![allow(deprecated)]

pub mod conductor;
#[allow(missing_docs)]
pub mod core;
#[allow(missing_docs)]
pub mod fixt;
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
