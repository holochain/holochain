use holochain_types::dht_op::DhtOpType;
use holochain_zome_types::HeaderType;
use thiserror::Error;
#[derive(Error, Debug)]
pub enum StateQueryError {
    #[error(transparent)]
    Sql(#[from] holochain_sqlite::rusqlite::Error),
    #[error(transparent)]
    Infallible(#[from] std::convert::Infallible),
    #[error(transparent)]
    DatabaseError(#[from] holochain_sqlite::error::DatabaseError),
    #[error(transparent)]
    SerializedBytesError(#[from] holochain_serialized_bytes::SerializedBytesError),
    #[error(transparent)]
    DhtOpError(#[from] holochain_types::dht_op::error::DhtOpError),
    #[error("Unexpected op {0:?} for query")]
    UnexpectedOp(DhtOpType),
    #[error("Unexpected header {0:?} for query")]
    UnexpectedHeader(HeaderType),
    #[error(transparent)]
    WrongHeaderError(#[from] holochain_zome_types::WrongHeaderError),
    #[error(transparent)]
    HeaderError(#[from] holochain_types::header::error::HeaderError),
}

pub type StateQueryResult<T> = Result<T, StateQueryError>;
