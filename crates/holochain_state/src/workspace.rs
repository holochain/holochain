//! Workspaces are a simple abstraction used to stage changes during Workflow
//! execution to be persisted later
//!
//! Every Workflow has an associated Workspace type.

use super::source_chain::SourceChainError;
use holochain_sqlite::error::DatabaseError;
use thiserror::Error;

#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum WorkspaceError {
    #[error(transparent)]
    DatabaseError(#[from] DatabaseError),

    #[error(transparent)]
    SourceChainError(#[from] SourceChainError),

    #[error(transparent)]
    StateQueryError(#[from] crate::query::StateQueryError),

    #[error(transparent)]
    StateMutationError(#[from] crate::mutations::StateMutationError),
}

#[allow(missing_docs)]
pub type WorkspaceResult<T> = Result<T, WorkspaceError>;
