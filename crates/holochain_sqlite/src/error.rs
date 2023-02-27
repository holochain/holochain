//! All possible errors when working with SQLite databases

// missing_docs allowed here since the errors already have self-descriptive strings
#![allow(missing_docs)]

use holochain_serialized_bytes::SerializedBytesError;
use holochain_zome_types::block::BlockError;
use holochain_zome_types::TimestampError;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("A database's database map was initialized more than once: {0}")]
    EnvironmentDoubleInitialized(PathBuf),

    #[error("database directory does not exist at configured path: {0}")]
    DatabaseMissing(PathBuf),

    #[error(
        "Attempted to access a private entry in a context where no private database is specified: {0}"
    )]
    NoPrivateDb(String),

    #[error("Error encoding to MsgPack: {0}")]
    MsgPackEncodeError(#[from] rmp_serde::encode::Error),

    #[error("Error decoding to MsgPack: {0}")]
    MsgPackDecodeError(#[from] rmp_serde::decode::Error),

    #[error("SerializedBytes error when attempting to interact with SQLite: {0}")]
    SerializedBytes(#[from] SerializedBytesError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),

    #[error(transparent)]
    SqliteError(#[from] rusqlite::Error),

    #[error("Failure to remove directory")]
    DirectoryError(#[from] std::io::Error),

    #[error(transparent)]
    DbConnectionPoolError(#[from] r2d2::Error),

    #[error("Empty keys cannot be used with SQLite")]
    EmptyKey,

    #[error("Key range must be not empty and start < end")]
    InvalidKeyRange,

    #[error("Unable to construct a value key")]
    KeyConstruction,

    #[error("transparent")]
    FailedToJoinBlocking(#[from] tokio::task::JoinError),

    #[error(transparent)]
    Timestamp(TimestampError),

    #[error(transparent)]
    GetRandom(getrandom::Error),

    #[error(transparent)]
    Block(BlockError),
}

impl From<BlockError> for DatabaseError {
    fn from(block_error: BlockError) -> Self {
        Self::Block(block_error)
    }
}

impl From<TimestampError> for DatabaseError {
    fn from(timestamp_error: TimestampError) -> Self {
        Self::Timestamp(timestamp_error)
    }
}

impl From<getrandom::Error> for DatabaseError {
    fn from(getrandom_error: getrandom::Error) -> Self {
        Self::GetRandom(getrandom_error)
    }
}

impl PartialEq for DatabaseError {
    fn eq(&self, other: &Self) -> bool {
        self.to_string() == other.to_string()
    }
}

pub type DatabaseResult<T> = Result<T, DatabaseError>;
