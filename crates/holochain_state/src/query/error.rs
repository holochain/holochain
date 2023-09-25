use holochain_types::dht_op::DhtOpType;
use holochain_zome_types::ActionType;
use thiserror::Error;

use crate::scratch::SyncScratchError;
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
    #[error("Unexpected action {0:?} for query")]
    UnexpectedAction(ActionType),
    #[error(transparent)]
    WrongActionError(#[from] holochain_zome_types::WrongActionError),
    #[error(transparent)]
    ActionError(#[from] holochain_zome_types::action::ActionError),
    #[error(transparent)]
    SyncScratchError(#[from] SyncScratchError),
}

pub type StateQueryResult<T> = Result<T, StateQueryError>;
