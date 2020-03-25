//! All possible errors when working with LMDB databases

// missing_docs allowed here since the errors already have self-descriptive strings
#![allow(missing_docs)]

use crate::db::DbName;
use failure::Fail;
use std::{path::PathBuf, backtrace::Backtrace};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("A store which was expected not to be empty turned out to be empty: {0}")]
    EmptyStore(DbName),

    #[error("An LMDB store was not created/initialized: {0}")]
    StoreNotInitialized(DbName),

    #[error("LMDB environment directory does not exist at configured path: {0}")]
    EnvironmentMissing(PathBuf),

    #[error("There is an unexpected value in an LMDB database (TODO: more info)")]
    InvalidValue,

    #[error("Error interacting with the underlying LMDB store: {source}")]
    LmdbStoreError {
        #[from]
        source: failure::Compat<rkv::StoreError>,
        backtrace: Backtrace,
    },

    #[error("Error when attempting an LMDB data transformation: {source}")]
    LmdbDataError {
        #[from]
        source: failure::Compat<rkv::DataError>,
        backtrace: Backtrace,
    },

    #[error("Error encoding to MsgPack: {0}")]
    MsgPackEncodeError(#[from] rmp_serde::encode::Error),

    #[error("Error decoding to MsgPack: {0}")]
    MsgPackDecodeError(#[from] rmp_serde::decode::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl PartialEq for DatabaseError {
    fn eq(&self, other: &Self) -> bool {
        self.to_string() == other.to_string()
    }
}

pub type DatabaseResult<T> = Result<T, DatabaseError>;

// Note: these are necessary since rkv Errors do not have std::Error impls,
// so we have to do some finagling

impl From<rkv::StoreError> for DatabaseError {
    fn from(e: rkv::StoreError) -> DatabaseError {
        DatabaseError::LmdbStoreError {
            source: e.compat(),
            backtrace: Backtrace::capture(),
        }
    }
}

impl From<rkv::DataError> for DatabaseError {
    fn from(e: rkv::DataError) -> DatabaseError {
        DatabaseError::LmdbDataError {
            source: e.compat(),
            backtrace: Backtrace::capture(),
        }
    }
}
