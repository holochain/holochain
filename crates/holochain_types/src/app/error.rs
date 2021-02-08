#![allow(missing_docs)]

use super::CellSlot;
use crate::prelude::*;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Clone limit of {0} exceeded for cell: {1:?}")]
    CloneLimitExceeded(u32, CellSlot),

    #[error("Tried to access missing cell nick: '{0}'")]
    CellNickMissing(CellNick),
}
pub type AppResult<T> = Result<T, AppError>;
