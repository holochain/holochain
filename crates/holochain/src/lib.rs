//! All the components you need to build a Holochain Conductor

// Toggle this to see what needs to be eventually refactored (as warnings).
#![allow(deprecated)]
// We have a lot of usages of type aliases to `&String`, which clippy objects to.
#![allow(clippy::ptr_arg)]

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
