#![deny(missing_docs)]
//! Errors occurring during a [Ribosome] call

use holochain_wasmer_host::prelude::WasmError;
use sx_types::dna::error::DnaError;
use thiserror::Error;

/// Errors occurring during a [Ribosome] call
#[derive(Error, Debug)]
pub enum RibosomeError {
    /// Dna error while working with Ribosome.
    #[error("Dna error while working with Ribosome: {0}")]
    DnaError(#[from] DnaError),

    /// Wasm error while working with Ribosome.
    #[error("Wasm error while working with Ribosome: {0}")]
    WasmError(#[from] WasmError),
}

/// Type alias
pub type RibosomeResult<T> = Result<T, RibosomeError>;
