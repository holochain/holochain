// Error types are self-explanatory
#![allow(missing_docs)]

use super::app_validation_workflow::AppValidationError;
use crate::conductor::api::error::ConductorApiError;
use crate::conductor::CellError;
use crate::core::queue_consumer::QueueTriggerClosedError;
use crate::core::ribosome::error::RibosomeError;
use crate::core::SysValidationError;
use holochain_cascade::error::CascadeError;
use holochain_keystore::KeystoreError;
use holochain_p2p::HolochainP2pError;
use holochain_sqlite::error::DatabaseError;
use holochain_state::source_chain::SourceChainError;
use holochain_types::prelude::*;
use thiserror::Error;

#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum WorkflowError {
    #[error("The genesis self-check failed. App cannot be installed. Reason: {0}")]
    GenesisFailure(String),

    #[error(transparent)]
    AppValidationError(#[from] AppValidationError),

    #[error("Agent is invalid: {0:?}")]
    AgentInvalid(AgentPubKey),

    #[error("Conductor API error: {0}")]
    ConductorApi(#[from] Box<ConductorApiError>),

    #[error(transparent)]
    CascadeError(#[from] CascadeError),

    #[error(transparent)]
    CounterSigningError(#[from] CounterSigningError),

    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),

    #[error(transparent)]
    RibosomeError(#[from] RibosomeError),

    #[error("Source chain error: {0}")]
    SourceChainError(#[from] SourceChainError),

    #[error("Capability token missing")]
    CapabilityMissing,

    #[error(transparent)]
    SerializedBytesError(#[from] SerializedBytesError),

    #[error(transparent)]
    CellError(#[from] CellError),

    #[error(transparent)]
    JoinError(#[from] tokio::task::JoinError),

    #[error(transparent)]
    QueueTriggerClosedError(#[from] QueueTriggerClosedError),

    #[error(transparent)]
    HolochainP2pError(#[from] HolochainP2pError),

    #[error(transparent)]
    HoloHashError(#[from] holo_hash::HoloHashError),

    #[error(transparent)]
    InterfaceError(#[from] crate::conductor::interface::error::InterfaceError),

    #[error(transparent)]
    DhtOpError(#[from] DhtOpError),

    #[error(transparent)]
    SysValidationError(#[from] SysValidationError),

    #[error(transparent)]
    KeystoreError(#[from] KeystoreError),

    #[error(transparent)]
    SqlError(#[from] holochain_sqlite::rusqlite::Error),

    #[error(transparent)]
    StateQueryError(#[from] holochain_state::query::StateQueryError),

    #[error(transparent)]
    StateMutationError(#[from] holochain_state::mutations::StateMutationError),

    #[error(transparent)]
    SystemTimeError(#[from] std::time::SystemTimeError),

    #[error(transparent)]
    TimestampError(#[from] holochain_zome_types::prelude::TimestampError),

    #[error("RecvError")]
    RecvError,

    #[error(transparent)]
    SendError(#[from] tokio::sync::mpsc::error::SendError<()>),

    /// Other
    #[error("Other: {0}")]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl WorkflowError {
    /// promote a custom error type to a WorkflowError
    pub fn other(e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::Other(e.into())
    }

    /// True if a workflow encountering this error should bail, else it should
    /// continue executing/looping.
    pub fn workflow_should_bail(&self) -> bool {
        // Currently GenesisFailure is the only thing we abort the app for but
        // in the future this could be expanded to a more sophisticated match
        // statement covering more fatal issues.
        matches!(self, Self::GenesisFailure(_))
    }
}

impl From<one_err::OneErr> for WorkflowError {
    fn from(e: one_err::OneErr) -> Self {
        Self::other(e)
    }
}

/// Internal type to handle running workflows
pub type WorkflowResult<T> = Result<T, WorkflowError>;
