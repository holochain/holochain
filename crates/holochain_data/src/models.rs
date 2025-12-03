//! Database models for Holochain data structures.
//! This module provides data models for the various Holochain database schemas.

/// WASM-related models (WASM bytecode, DNA definitions, zomes, entry definitions)
pub mod wasm;

// Re-export WASM models for convenience
pub use wasm::{CoordinatorZomeModel, DnaDefModel, EntryDefModel, IntegrityZomeModel, WasmModel};
