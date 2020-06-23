#![deny(missing_docs)]
//! Errors occurring during a [Ribosome] call

use holochain_crypto::CryptoError;
use holochain_serialized_bytes::prelude::SerializedBytesError;
use holochain_types::dna::error::DnaError;
use holochain_wasmer_host::prelude::WasmError;
use holochain_zome_types::zome::ZomeName;
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

    /// Serialization error while working with Ribosome.
    #[error("Serialization error while working with Ribosome: {0}")]
    SerializationError(#[from] SerializedBytesError),

    /// A ZomeFn was called by name that doesn't exist
    #[error("Attempted to call a zome function that doesn't exist: Zome: {0} Fn {1}")]
    ZomeFnNotExists(ZomeName, String),

    /// ident
    #[error(transparent)]
    CryptoError(#[from] CryptoError),

    /// ident
    #[error(transparent)]
    DatabaseError(#[from] holochain_state::error::DatabaseError),
}

/// Type alias
pub type RibosomeResult<T> = Result<T, RibosomeError>;
