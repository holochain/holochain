use thiserror::Error;

#[derive(Error, Debug)]
pub enum CommitGroupError {
    #[error("Created a CommitGroup without an entry")]
    MissingEntry,
    #[error("Created a CommitGroup with an action without entry data")]
    MissingEntryData,
    #[error("Created a CommitGroup with no actions")]
    Empty,
}

pub type CommitGroupResult<T> = Result<T, CommitGroupError>;
