use thiserror::Error;

#[derive(Error, Debug)]
pub enum HeaderError {
    #[error(
        "Tried to create a NewEntryHeader with a type that isn't an CreateEntry or UpdateEntry"
    )]
    NotNewEntry,
}
