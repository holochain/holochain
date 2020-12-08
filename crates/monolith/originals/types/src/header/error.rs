use holochain_zome_types::header::conversions::WrongHeaderError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HeaderError {
    #[error("Tried to create a NewEntryHeader with a type that isn't a Create or Update")]
    NotNewEntry,
    #[error(transparent)]
    WrongHeaderError(#[from] WrongHeaderError),
}
