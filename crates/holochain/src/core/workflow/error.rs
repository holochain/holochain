// Error types are self-explanatory
#![allow(missing_docs)]

use super::{produce_dht_op_workflow::dht_op::error::DhtOpConvertError, Workflow, WorkflowEffects};
use crate::{
    conductor::{api::error::ConductorApiError, CellError},
    core::{
        ribosome::error::RibosomeError,
        state::{source_chain::SourceChainError, workspace::WorkspaceError},
    },
};
use holochain_state::error::DatabaseError;
use holochain_types::prelude::*;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorkflowError {
    #[error("Agent is invalid: {0:?}")]
    AgentInvalid(AgentPubKey),

    #[error("Conductor API error: {0}")]
    ConductorApi(#[from] Box<ConductorApiError>),

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
    FailedToHash(#[from] SerializedBytesError),

    #[error(transparent)]
    DhtOpConvertError(#[from] DhtOpConvertError),
}

/// The `Result::Ok` of any workflow function is
/// a tuple of the function output and a `WorkflowEffects` struct.
pub type WorkflowResult<'env, Wf> = Result<
    (
        <Wf as Workflow<'env>>::Output,
        WorkflowEffects<<Wf as Workflow<'env>>::Workspace, <Wf as Workflow<'env>>::Triggers>,
    ),
    WorkflowError,
>;

#[derive(Error, Debug)]
pub enum WorkflowRunError {
    #[error(transparent)]
    DatabaseError(#[from] DatabaseError),

    #[error(transparent)]
    WorkflowError(#[from] WorkflowError),

    #[error(transparent)]
    WorkspaceError(#[from] WorkspaceError),

    #[error(transparent)]
    CellError(#[from] CellError),
}

/// Internal type to handle running workflows
pub type WorkflowRunResult<T> = Result<T, WorkflowRunError>;
