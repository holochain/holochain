//! All possible errors when working with LMDB databases

// missing_docs allowed here since the errors already have self-descriptive strings
#![allow(missing_docs)]

use crate::prelude::*;
use crate::table::TableName;
use holochain_serialized_bytes::SerializedBytesError;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("A store which was expected not to be empty turned out to be empty: {0}")]
    EmptyStore(TableName),

    #[error("An LMDB store was not created/initialized: {0}, path: {1}")]
    StoreNotInitialized(TableName, PathBuf),

    #[error("An LMDB environment's database map was initialized more than once: {0}")]
    EnvironmentDoubleInitialized(PathBuf),

    #[error("LMDB environment directory does not exist at configured path: {0}")]
    EnvironmentMissing(PathBuf),

    #[error("There is an unexpected value in an LMDB database (TODO: more info)")]
    InvalidValue,

    #[error(
        "Attempted to access a private entry in a context where no private database is specified: {0}"
    )]
    NoPrivateDb(String),

    #[error(transparent)]
    ShimStoreError(#[from] StoreError),

    #[error("Error encoding to MsgPack: {0}")]
    MsgPackEncodeError(#[from] rmp_serde::encode::Error),

    #[error("Error decoding to MsgPack: {0}")]
    MsgPackDecodeError(#[from] rmp_serde::decode::Error),

    #[error("SerializedBytes error when attempting to interact with LMDB: {0}")]
    SerializedBytes(#[from] SerializedBytesError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),

    #[error(transparent)]
    SqlLiteError(#[from] rusqlite::Error),

    #[error("Failue to remove directory")]
    DirectoryError(#[from] std::io::Error),

    #[error(transparent)]
    KeystoreError(#[from] holochain_keystore::KeystoreError),

    #[error("Empty keys cannot be used with lmdb")]
    EmptyKey,

    #[error("Key range must be not empty and start < end")]
    InvalidKeyRange,

    #[error("Unable to construct a value key")]
    KeyConstruction,
}

impl PartialEq for DatabaseError {
    fn eq(&self, other: &Self) -> bool {
        self.to_string() == other.to_string()
    }
}

impl DatabaseError {
    pub fn ok_if_not_found(self) -> DatabaseResult<()> {
        todo!("implement for rusqlite errors")
        // match self {
        //     DatabaseError::LmdbStoreError(err) => match err.into_inner() {
        //         rkv::StoreError::LmdbError(rkv::LmdbError::NotFound) => Ok(()),
        //         err => Err(err.into()),
        //     },
        //     err => Err(err),
        // }
    }
}

pub type DatabaseResult<T> = Result<T, DatabaseError>;
