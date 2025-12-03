//! Database models for Holochain data structures.
//!
//! These models represent the database schema and may differ from the
//! corresponding types in `holochain_types` or `holochain_zome_types`.
//! The models are designed to be flat and easily mappable to SQL tables.

/// WASM-related models (WASM bytecode, DNA definitions, zomes, entry definitions)
pub mod wasm;

// Re-export WASM models for convenience
pub use wasm::{CoordinatorZomeModel, DnaDefModel, EntryDefModel, IntegrityZomeModel, WasmModel};
