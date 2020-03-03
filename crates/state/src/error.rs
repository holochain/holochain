use crate::db::DbName;
use failure::Fail;
use std::backtrace::Backtrace;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorkspaceError {
    #[error("A store which was expected not to be empty turned out to be empty: {0}")]
    EmptyStore(DbName),

    #[error("An LMDB store was not created/initialized: {0}")]
    StoreNotInitialized(DbName),

    /// This is an error we can catch and treat specially
    #[error("The source chain head has moved since this store was created")]
    SourceChainHeadMoved,

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
}

impl PartialEq for WorkspaceError {
    fn eq(&self, other: &Self) -> bool {
        self.to_string() == other.to_string()
    }
}

pub type WorkspaceResult<T> = Result<T, WorkspaceError>;

// Note: these are necessary since rkv Errors do not have std::Error impls,
// so we have to do some finagling

impl From<rkv::StoreError> for WorkspaceError {
    fn from(e: rkv::StoreError) -> WorkspaceError {
        WorkspaceError::LmdbStoreError {
            source: e.compat(),
            backtrace: Backtrace::capture(),
        }
    }
}

impl From<rkv::DataError> for WorkspaceError {
    fn from(e: rkv::DataError) -> WorkspaceError {
        WorkspaceError::LmdbDataError {
            source: e.compat(),
            backtrace: Backtrace::capture(),
        }
    }
}
