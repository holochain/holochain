#![deny(missing_docs)]
//! Errors occurring during a [Ribosome] call

use crate::conductor::api::error::ConductorApiError;
use crate::conductor::interface::error::InterfaceError;
use crate::core::workflow::produce_dht_ops_workflow::dht_op_light::error::DhtOpConvertError;
use holo_hash::AnyDhtHash;
use holochain_cascade::error::CascadeError;
use holochain_serialized_bytes::prelude::SerializedBytesError;
use holochain_state::source_chain::SourceChainError;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use thiserror::Error;
use tokio::task::JoinError;
use tokio_safe_block_on::BlockOnError;

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

    /// A Zome was referenced by name that doesn't exist
    #[error("Referenced a zome that doesn't exist: Zome: {0}")]
    ZomeNotExists(ZomeName),

    /// A ZomeFn was called by name that doesn't exist
    #[error("Attempted to call a zome function that doesn't exist: Zome: {0} Fn {1}")]
    ZomeFnNotExists(ZomeName, FunctionName),

    /// a problem with entry defs
    #[error("An error with entry defs: {0}")]
    EntryDefs(ZomeName, String),

    /// a mandatory dependency for an element doesn't exist
    /// for example a remove link ribosome call needs to find the add link in order to infer the
    /// correct base and this dependent relationship exists before even subconscious validation
    /// kicks in
    #[error("A mandatory element is missing, dht hash: {0}")]
    ElementDeps(AnyDhtHash),

    /// ident
    #[error("Unspecified ring error")]
    RingUnspecified,

    /// ident
    #[error(transparent)]
    KeystoreError(#[from] holochain_keystore::KeystoreError),

    /// ident
    #[error(transparent)]
    DatabaseError(#[from] holochain_lmdb::error::DatabaseError),

    /// ident
    #[error(transparent)]
    CascadeError(#[from] CascadeError),

    /// ident
    #[error(transparent)]
    ConductorApiError(#[from] Box<ConductorApiError>),

    /// ident
    #[error(transparent)]
    SourceChainError(#[from] SourceChainError),

    /// ident
    #[error(transparent)]
    InterfaceError(#[from] InterfaceError),

    /// ident
    #[error(transparent)]
    BlockOnError(#[from] BlockOnError),

    /// ident
    #[error(transparent)]
    JoinError(#[from] JoinError),

    /// ident
    #[error(transparent)]
    InlineZomeError(#[from] InlineZomeError),

    /// ident
    #[error(transparent)]
    P2pError(#[from] holochain_p2p::HolochainP2pError),

    /// ident
    #[error(transparent)]
    DhtOpConvertError(#[from] Box<DhtOpConvertError>),

    /// ident
    #[error("xsalsa20poly1305 error {0}")]
    Aead(String),

    /// ident
    #[error(transparent)]
    SecurePrimitive(#[from] holochain_zome_types::SecurePrimitiveError),
}

impl From<xsalsa20poly1305::aead::Error> for RibosomeError {
    fn from(error: xsalsa20poly1305::aead::Error) -> Self {
        Self::Aead(error.to_string())
    }
}

impl From<ring::error::Unspecified> for RibosomeError {
    fn from(_: ring::error::Unspecified) -> Self {
        Self::RingUnspecified
    }
}

/// Type alias
pub type RibosomeResult<T> = Result<T, RibosomeError>;
