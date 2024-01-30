//! Errors occurring during a [`RealRibosome`](crate::core::ribosome::real_ribosome::RealRibosome) call

use crate::conductor::api::error::ConductorApiError;
use crate::conductor::interface::error::InterfaceError;
use holo_hash::AnyDhtHash;
use holochain_cascade::error::CascadeError;
use holochain_secure_primitive::SecurePrimitiveError;
use holochain_serialized_bytes::prelude::SerializedBytesError;
use holochain_state::source_chain::SourceChainError;
use holochain_types::prelude::*;
use thiserror::Error;
use tokio::task::JoinError;
use wasmer::DeserializeError;

/// Errors occurring during a [`RealRibosome`](crate::core::ribosome::real_ribosome::RealRibosome) call
#[derive(Error, Debug)]
pub enum RibosomeError {
    /// Dna error while working with Ribosome.
    #[error("Dna error while working with Ribosome: {0}")]
    DnaError(#[from] DnaError),

    /// Wasm runtime error while working with Ribosome.
    #[error("Wasm runtime error while working with Ribosome: {0}")]
    WasmRuntimeError(#[from] wasmer::RuntimeError),

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
    #[error("An error with entry defs in zome '{0}': {1}")]
    EntryDefs(ZomeName, String),

    /// a mandatory dependency for a record doesn't exist
    /// for example a remove link ribosome call needs to find the add link in order to infer the
    /// correct base and this dependent relationship exists before even subconscious validation
    /// kicks in
    #[error("A mandatory record is missing, dht hash: {0}")]
    RecordDeps(AnyDhtHash),

    /// ident
    #[error(transparent)]
    KeystoreError(#[from] holochain_keystore::KeystoreError),

    /// ident
    #[error(transparent)]
    DatabaseError(#[from] holochain_sqlite::error::DatabaseError),

    /// ident
    #[error(transparent)]
    StateQueryError(#[from] holochain_state::query::StateQueryError),

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
    JoinError(#[from] JoinError),

    /// ident
    #[error(transparent)]
    InlineZomeError(#[from] InlineZomeError),

    /// ident
    #[error(transparent)]
    P2pError(#[from] holochain_p2p::HolochainP2pError),

    /// ident
    #[error(transparent)]
    SecurePrimitive(#[from] SecurePrimitiveError),

    /// Zome function doesn't have permissions to call a Host function.
    #[error("Host function {2} cannot be called from zome function {1} in zome {0}")]
    HostFnPermissions(ZomeName, FunctionName, String),

    /// An attempt to call a host function that changes the state of an installed app from a cell that isn't part of that app.
    #[error("Invalid request to modify cell {0} with a zome call to {1}")]
    CrossCellConductorCall(CellId, CellId),

    #[error(transparent)]
    ZomeTypesError(#[from] holochain_types::zome_types::ZomeTypesError),

    #[error(transparent)]
    ModuleDeserializeError(#[from] DeserializeError),

    #[error(transparent)]
    IO(#[from] std::io::Error),
}

/// Type alias
pub type RibosomeResult<T> = Result<T, RibosomeError>;
