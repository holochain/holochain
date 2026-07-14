#![allow(missing_docs)]

use holochain_p2p::HolochainP2pError;
use holochain_types::prelude::*;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CascadeError {
    #[error(transparent)]
    DhtOpError(#[from] DhtOpError),

    #[error("Got an invalid response from an authority for the request hash: {0:?}")]
    InvalidResponse(AnyDhtHash),

    #[error("Input parameters are invalid: {0}")]
    InvalidInput(String),

    #[error(transparent)]
    NetworkError(#[from] HolochainP2pError),

    #[error(transparent)]
    QueryError(#[from] holochain_state::query::StateQueryError),

    #[error(transparent)]
    StateMutationError(#[from] holochain_state::mutations::StateMutationError),

    #[error("Network not initialized")]
    NetworkNotInitialized,
}

pub type CascadeResult<T> = Result<T, CascadeError>;
