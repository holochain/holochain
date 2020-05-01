use super::WorkflowEffects;
use crate::{conductor::api::error::ConductorApiError, core::state::{source_chain::SourceChainError, workspace::WorkspaceError}};
use holochain_state::error::DatabaseError;
use holochain_types::prelude::*;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorkflowError {
    #[error("Agent is invalid: {0:?}")]
    AgentInvalid(AgentHash),

    #[error("Conductor API error: {0}")]
    ConductorApi(#[from] ConductorApiError),

    #[error("Workspace error: {0}")]
    WorkspaceError(#[from] WorkspaceError),

    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),

    #[error("Source chain error: {0}")]
    SourceChainError(#[from] SourceChainError),
}

/// The `Result::Ok` of any workflow function is
/// a tuple of the function output and a `WorkflowEffects` struct.
pub type WorkflowResult<'env, O, WC> = Result<(O, WorkflowEffects<'env, WC>), WorkflowError>;

#[derive(Error, Debug)]
pub enum WorkflowRunError {
    #[error(transparent)]
    DatabaseError(#[from] DatabaseError),

    #[error(transparent)]
    WorkflowError(#[from] WorkflowError),

    #[error(transparent)]
    WorkspaceError(#[from] WorkspaceError),
}

/// Internal type to handle running workflows
pub type WorkflowRunResult<T: Sized> = Result<T, WorkflowRunError>;
