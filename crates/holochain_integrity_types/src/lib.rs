//! Holochain Integrity Types: only the types needed by Holochain application
//! developers to use in their Zome code, and nothing more.
//!
//! This crate is intentionally kept as minimal as possible, since it is
//! typically included as a dependency in Holochain Zomes, which are
//! distributed as chunks of Wasm.

#![deny(missing_docs)]

#[allow(missing_docs)]
pub mod link;
pub mod prelude;
pub mod zome;
