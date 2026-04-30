//! Operations for the Wasm database.
//!
//! The Wasm database stores DNA definitions, WASM bytecode, and entry definitions.

mod inner_reads;
mod inner_writes;
mod reads;
mod writes;

pub mod db_operations;
pub mod tx_operations;
