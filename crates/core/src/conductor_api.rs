use crate::{
    cell::{autonomic::AutonomicCue, Cell, error::CellError},
    nucleus::{ZomeInvocation, ZomeInvocationResult},
};
use async_trait::async_trait;
use sx_types::{
    shims::{Lib3hClientProtocol, Lib3hServerProtocol},
    signature::Signature,
};
use thiserror::Error;

/// The interface for a Cell to talk to its calling Conductor
#[async_trait(?Send)]
pub trait ConductorCellApiT {
    async fn invoke_zome(
        &self,
        cell: Cell,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResult>;

    async fn network_send(&self, message: Lib3hClientProtocol) -> ConductorApiResult<()>;

    async fn network_request(
        &self,
        _message: Lib3hClientProtocol,
    ) -> ConductorApiResult<Lib3hServerProtocol>;

    async fn autonomic_cue(&self, cue: AutonomicCue) -> ConductorApiResult<()>;

    async fn crypto_sign(&self, _payload: String) -> ConductorApiResult<Signature>;

    async fn crypto_encrypt(&self, _payload: String) -> ConductorApiResult<String>;

    async fn crypto_decrypt(&self, _payload: String) -> ConductorApiResult<String>;
}

#[derive(Error, Debug)]
pub enum ConductorApiError {

    #[error("CellError: {0}")]
    CellError(#[from] CellError),

    /// Since ConductorError is defined in an upstream crate, we can't use it here.
    /// In particular, the trait and the impl for ConductorCellApiT are defined in
    /// this crate and the upstream conductor_api crate, respectively.
    /// We could break the loop by moving both error types downstream, but let's wait and see.
    #[error("Got an error from the Conductor, but lost all error context (TODO): {0}")]
    ConductorInceptionError(String),

    #[error("Miscellaneous error: {0}")]
    Misc(String),
}

pub type ConductorApiResult<T> = Result<T, ConductorApiError>;
