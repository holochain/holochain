use super::error::{ConductorApiError, ConductorApiResult};
use crate::conductor::{cell::Cell, conductor::Conductor};
use async_trait::async_trait;
use std::sync::Arc;
use sx_types::{
    autonomic::AutonomicCue,
    cell::CellId,
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
    shims::*,
    signature::Signature,
};
use tokio::sync::RwLock;

/// The concrete implementation of [CellConductorApiT], which is used to give
/// Cells an API for calling back to their [Conductor].
#[derive(Clone)]
pub struct CellConductorApi {
    lock: Arc<RwLock<Conductor>>,
    cell_id: CellId,
}

impl CellConductorApi {
    /// Instantiate from a Conductor reference and a CellId to identify which Cell
    /// this API instance is associated with
    pub fn new(lock: Arc<RwLock<Conductor>>, cell_id: CellId) -> Self {
        Self { cell_id, lock }
    }
}

#[async_trait]
impl CellConductorApiT for CellConductorApi {
    async fn invoke_zome(
        &self,
        cell_id: &CellId,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse> {
        let conductor = self.lock.read().await;
        let cell: &Cell = conductor.cell_by_id(cell_id)?;
        cell.invoke_zome(self.clone(), invocation)
            .await
            .map_err(Into::into)
    }

    async fn network_send(&self, message: Lib3hClientProtocol) -> ConductorApiResult<()> {
        let mut tx = self.lock.read().await.tx_network().clone();
        tx.send(message)
            .await
            .map_err(|e| ConductorApiError::Misc(e.to_string()))
    }

    async fn network_request(
        &self,
        _message: Lib3hClientProtocol,
    ) -> ConductorApiResult<Lib3hServerProtocol> {
        unimplemented!()
    }

    async fn autonomic_cue(&self, cue: AutonomicCue) -> ConductorApiResult<()> {
        let conductor = self.lock.write().await;
        let cell = conductor.cell_by_id(&self.cell_id)?;
        let _ = cell.handle_autonomic_process(cue.into()).await;
        Ok(())
    }

    async fn crypto_sign(&self, _payload: String) -> ConductorApiResult<Signature> {
        unimplemented!()
    }

    async fn crypto_encrypt(&self, _payload: String) -> ConductorApiResult<String> {
        unimplemented!()
    }

    async fn crypto_decrypt(&self, _payload: String) -> ConductorApiResult<String> {
        unimplemented!()
    }

    async fn dpki_request(&self, _method: String, _args: String) -> ConductorApiResult<String> {
        unimplemented!()
    }
}

/// The "internal" Conductor API interface, for a Cell to talk to its calling Conductor.
#[async_trait]
pub trait CellConductorApiT: Clone + Send + Sync + Sized {

    /// Invoke a zome function on any cell in this conductor.
    /// An invocation on a different Cell than this one corresponds to a bridged call.
    async fn invoke_zome(
        &self,
        cell_id: &CellId,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse>;

    /// Send a message to the network engine, ignoring the response
    async fn network_send(&self, message: Lib3hClientProtocol) -> ConductorApiResult<()>;

    /// Send a message to the network engine, and await the response
    async fn network_request(
        &self,
        _message: Lib3hClientProtocol,
    ) -> ConductorApiResult<Lib3hServerProtocol>;

    /// Cue the autonomic system to run an [AutonomicProcess] earlier than its scheduled time.
    /// This is basically a heuristic designed to help things run more smoothly.
    async fn autonomic_cue(&self, cue: AutonomicCue) -> ConductorApiResult<()>;

    /// Request the crypto system to sign some payload
    /// TODO: decide on actual signature
    async fn crypto_sign(&self, _payload: String) -> ConductorApiResult<Signature>;

    /// Request the crypto system to encrypt some payload
    /// TODO: decide on actual signature
    async fn crypto_encrypt(&self, _payload: String) -> ConductorApiResult<String>;

    /// Request the crypto system to decrypt some payload
    /// TODO: decide on actual signature
    async fn crypto_decrypt(&self, _payload: String) -> ConductorApiResult<String>;

    /// Make a request to the DPKI service running for this Conductor.
    /// TODO: decide on actual signature
    async fn dpki_request(&self, method: String, args: String) -> ConductorApiResult<String>;
}
