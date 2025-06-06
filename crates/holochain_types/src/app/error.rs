#![allow(missing_docs)]

use crate::prelude::*;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Clone limit of {0} exceeded for app role assignment: {1:?}")]
    CloneLimitExceeded(u32, Box<AppRolePrimary>),

    #[error("Tried to create a cell with an existing id '{0}'")]
    DuplicateCellId(CellId),

    #[error("Tried to create a clone cell with existing clone cell id '{0}'")]
    DuplicateCloneIds(CloneId),

    #[error("Could not find clone cell with id '{0}'")]
    CloneCellNotFound(CloneCellId),

    #[error("Tried to delete a clone cell which was not already disabled: '{0}'")]
    CloneCellMustBeDisabledBeforeDeleting(CloneCellId),

    #[error("Illegal character '{CLONE_ID_DELIMITER}' used in role name: {0}")]
    IllegalRoleName(RoleName),

    #[error("Tried to access missing role name: '{0}'")]
    RoleNameMissing(RoleName),

    #[error("Tried to install app '{0}' which contains duplicate role names. The following role names have duplicates: {1:?}")]
    DuplicateRoleNames(InstalledAppId, Vec<RoleName>),

    #[error("Agent key '{0}' does not exist for app '{1}")]
    AgentKeyMissing(AgentPubKey, InstalledAppId),

    #[error("Tried to interact with a cell through a Dependency role assignment rather than the Primary assignment. Role name: '{0}'")]
    NonPrimaryCell(InstalledAppId, RoleName),
}
pub type AppResult<T> = Result<T, AppError>;
