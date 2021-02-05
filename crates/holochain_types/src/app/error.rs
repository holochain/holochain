#![allow(missing_docs)]

use super::CellSlot;
use crate::prelude::*;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Clone limit of {0} exceeded for cell: {1:?}")]
    CloneLimitExceeded(usize, CellSlot),

    #[error("Can't create clone for slot '{0}' because a clone with the same CellId already exists: {1}")]
    CloneAlreadyExists(CellNick, CellId),

    #[error("Tried to access missing cell nick: '{0}'")]
    CellNickMissing(CellNick),
}
pub type AppResult<T> = Result<T, AppError>;
