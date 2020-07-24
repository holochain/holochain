use holochain_p2p::HolochainP2pError;
use holochain_serialized_bytes::SerializedBytesError;
use holochain_state::error::DatabaseError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CascadeError {
    #[error(transparent)]
    DatabaseError(#[from] DatabaseError),

    #[error(transparent)]
    NetworkError(#[from] HolochainP2pError),

    #[error(transparent)]
    SerializedBytesError(#[from] SerializedBytesError),
}

pub type CascadeResult<T> = Result<T, CascadeError>;
