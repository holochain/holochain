use thiserror::Error;

#[derive(Error, Debug)]
pub enum ElementGroupError {
    #[error("Created an ElementGroup without an entry")]
    MissingEntry,
    #[error("Created an ElementGroup with an action without entry data")]
    MissingEntryData,
    #[error("Created an ElementGroup with no actions")]
    Empty,
}

pub type ElementGroupResult<T> = Result<T, ElementGroupError>;
