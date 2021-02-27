#![allow(missing_docs)]

use super::AppSlot;
use crate::prelude::*;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Clone limit of {0} exceeded for cell: {1:?}")]
    CloneLimitExceeded(u32, AppSlot),

    #[error("Tried to access missing slot id: '{0}'")]
    SlotIdMissing(SlotId),

    #[error("Tried to install app '{0}' which contains duplicate slot ids. The following slot ids have duplicates: {1:?}")]
    DuplicateSlotIds(InstalledAppId, Vec<SlotId>),
}
pub type AppResult<T> = Result<T, AppError>;
