use crate::core::{state::workspace::WorkspaceError, workflow::WorkflowError};
use sx_state::error::DatabaseError;
use thiserror::Error;

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
pub type WorkflowRunResult<T> = Result<T, WorkflowRunError>;
