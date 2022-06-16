use thiserror::Error;

#[derive(Error, Debug)]
pub enum RecordGroupError {
    #[error("Created a RecordGroup without an entry")]
    MissingEntry,
    #[error("Created a RecordGroup with an action without entry data")]
    MissingEntryData,
    #[error("Created a RecordGroup with no actions")]
    Empty,
}

pub type RecordGroupResult<T> = Result<T, RecordGroupError>;
