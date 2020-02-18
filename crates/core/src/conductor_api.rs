use crate::{
    cell::{autonomic::AutonomicCue, Cell, error::CellError, CellId},
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
        cell: CellId,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResult>;

    /// TODO: maybe move out into its own trait
    async fn network_send(&self, message: Lib3hClientProtocol) -> ConductorApiResult<()>;

    /// TODO: maybe move out into its own trait
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

use mockall::mock;
// mock


// Unfortunate workaround to get mockall to work with async_trait, due to the complexity of each.
// The mock! expansion here creates mocks on a non-async version of the API, and then the actual trait is implemented
// by delegating each async trait method to its sync counterpart
// See https://github.com/asomers/mockall/issues/75
mock! {
    pub ConductorCellApi {
        fn sync_invoke_zome(
            &self,
            cell_id: CellId,
            invocation: ZomeInvocation,
        ) -> ConductorApiResult<ZomeInvocationResult>;

        fn sync_network_send(&self, message: Lib3hClientProtocol) -> ConductorApiResult<()>;

        fn sync_network_request(
            &self,
            _message: Lib3hClientProtocol,
        ) -> ConductorApiResult<Lib3hServerProtocol>;

        fn sync_autonomic_cue(&self, cue: AutonomicCue) -> ConductorApiResult<()>;

        fn sync_crypto_sign(&self, _payload: String) -> ConductorApiResult<Signature>;

        fn sync_crypto_encrypt(&self, _payload: String) -> ConductorApiResult<String>;

        fn sync_crypto_decrypt(&self, _payload: String) -> ConductorApiResult<String>;
    }
}

#[async_trait(?Send)]
impl ConductorCellApiT for MockConductorCellApi {
    async fn invoke_zome(
        &self,
        cell_id: CellId,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResult> {
        self.sync_invoke_zome(cell_id, invocation)
    }

    async fn network_send(&self, message: Lib3hClientProtocol) -> ConductorApiResult<()> {
        self.sync_network_send(message)
    }

    async fn network_request(
        &self,
        _message: Lib3hClientProtocol,
    ) -> ConductorApiResult<Lib3hServerProtocol> {
        self.sync_network_request(_message)
    }

    async fn autonomic_cue(&self, cue: AutonomicCue) -> ConductorApiResult<()> {
        self.sync_autonomic_cue(cue)
    }

    async fn crypto_sign(&self, _payload: String) -> ConductorApiResult<Signature> {
        self.sync_crypto_sign(_payload)
    }

    async fn crypto_encrypt(&self, _payload: String) -> ConductorApiResult<String> {
        self.sync_crypto_encrypt(_payload)
    }

    async fn crypto_decrypt(&self, _payload: String) -> ConductorApiResult<String> {
        self.sync_crypto_decrypt(_payload)
    }
}
