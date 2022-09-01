#![allow(missing_docs)]

use super::AppRoleAssignment;
use crate::prelude::*;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Clone limit of {0} exceeded for app role assignment: {1:?}")]
    CloneLimitExceeded(u32, AppRoleAssignment),

    #[error("Tried to create a clone cell with existing clone cell id '{0}'")]
    DuplicateCloneIds(CloneId),

    #[error("Could not find clone cell with id '{0}'")]
    CloneCellNotFound(CloneId),

    #[error("Illegal character '{CLONE_ID_DELIMITER}' used in roled id: {0}")]
    IllegalRoleId(AppRoleId),

    #[error("Tried to access missing role id: '{0}'")]
    AppRoleIdMissing(AppRoleId),

    #[error("Tried to install app '{0}' which contains duplicate role ids. The following role ids have duplicates: {1:?}")]
    DuplicateAppRoleIds(InstalledAppId, Vec<AppRoleId>),
}
pub type AppResult<T> = Result<T, AppError>;
