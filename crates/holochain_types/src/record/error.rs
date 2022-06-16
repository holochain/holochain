use thiserror::Error;

#[derive(Error, Debug)]
pub enum RecordGroupError {
    #[error("Created an RecordGroup without an entry")]
    MissingEntry,
    #[error("Created an RecordGroup with an action without entry data")]
    MissingEntryData,
    #[error("Created an RecordGroup with no actions")]
    Empty,
}

pub type RecordGroupResult<T> = Result<T, RecordGroupError>;
