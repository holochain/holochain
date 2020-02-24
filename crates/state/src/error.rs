
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorkspaceError {

    #[error("There is an unexpected value in an LMDB database (TODO: more info)")]
    InvalidValue,

    #[error("Error interacting with the underlying LMDB store: {0}")]
    LmdbStoreError(rkv::StoreError),

    #[error("Error when attempting an LMDB data transformation: {0}")]
    LmdbDataError(rkv::DataError),

    #[error("Error encoding to MsgPack: {0}")]
    MsgPackEncodeError(#[from] rmp_serde::encode::Error),

    #[error("Error decoding to MsgPack: {0}")]
    MsgPackDecodeError(#[from] rmp_serde::decode::Error),
}

pub type WorkspaceResult<T> = Result<T, WorkspaceError>;

impl From<rkv::StoreError> for WorkspaceError {
    fn from(e: rkv::StoreError) -> WorkspaceError {
        WorkspaceError::LmdbStoreError(e)
    }
}

impl From<rkv::DataError> for WorkspaceError {
    fn from(e: rkv::DataError) -> WorkspaceError {
        WorkspaceError::LmdbDataError(e)
    }
}
