use crate::error::ConductorApiResult;
use sx_types::{
    autonomic::AutonomicCue,
    cell::CellId,
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
    shims::*,
    signature::Signature,
};

use async_trait::async_trait;

/// The "internal" Conductor API, for a Cell to talk to its calling Conductor
#[async_trait]
pub trait CellConductorApiT: Clone + Send + Sync + Sized {
    async fn invoke_zome(
        &self,
        cell_id: &CellId,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse>;

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

    async fn dpki_request(&self, method: String, args: String) -> ConductorApiResult<String>;
}
