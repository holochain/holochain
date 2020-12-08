// Error types are self-explanatory
#![allow(missing_docs)]

use super::app_validation_workflow::AppValidationError;
use super::produce_dht_ops_workflow::dht_op_light::error::DhtOpConvertError;
use crate::conductor::api::error::ConductorApiError;
use crate::conductor::CellError;
use crate::core::queue_consumer::QueueTriggerClosedError;
use crate::core::ribosome::error::RibosomeError;
use crate::core::state::cascade::error::CascadeError;
use crate::core::state::source_chain::SourceChainError;
use crate::core::state::workspace::WorkspaceError;
use crate::core::SysValidationError;
use monolith::holochain_p2p::HolochainP2pError;
use monolith::holochain_state::error::DatabaseError;
use monolith::holochain_types::dht_op::error::DhtOpError;
use monolith::holochain_types::prelude::*;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorkflowError {
    #[error(transparent)]
    AppValidationError(#[from] AppValidationError),

    #[error("Agent is invalid: {0:?}")]
    AgentInvalid(AgentPubKey),

    #[error("Conductor API error: {0}")]
    ConductorApi(#[from] Box<ConductorApiError>),

    #[error(transparent)]
    CascadeError(#[from] CascadeError),

    #[error("Workspace error: {0}")]
    WorkspaceError(#[from] WorkspaceError),

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
    DhtOpConvertError(#[from] DhtOpConvertError),

    #[error(transparent)]
    CellError(#[from] CellError),

    #[error(transparent)]
    QueueTriggerClosedError(#[from] QueueTriggerClosedError),

    #[error(transparent)]
    HolochainP2pError(#[from] HolochainP2pError),

    #[error(transparent)]
    DhtOpError(#[from] DhtOpError),

    #[error(transparent)]
    SysValidationError(#[from] SysValidationError),
}

/// Internal type to handle running workflows
pub type WorkflowResult<T> = Result<T, WorkflowError>;
