use sx_types::signature::Signature;
use sx_types::autonomic::AutonomicCue;
use sx_types::shims::*;
use sx_types::nucleus::ZomeInvocation;
use sx_types::nucleus::ZomeInvocationResponse;
use crate::error::ConductorApiResult;
use crate::cell::CellT;
use sx_types::agent::CellId;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::sync::Arc;
use crate::conductor::ConductorT;

use async_trait::async_trait;

/// The interface for a Cell to talk to its calling Conductor
#[async_trait(?Send)]
pub trait CellConductorInterfaceT: Clone + Send + Sync + Sized
{
    type Cell: CellT<Interface = Self>;
    type Conductor: ConductorT<Interface = Self>;

    // TODO: I realized late in the game that if all methods in this trait
    // are implemented only by the concrete type, then there is no need
    // for this trait to know about the type of the Conductor OR the Cell!
    // Might look into this to simplify things a lot...
    async fn conductor_ref(&self) -> RwLockReadGuard<Self::Conductor>;

    async fn invoke_zome(
        &self,
        cell_id: &CellId,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse> {
        let conductor = self.conductor_ref().await;
        let cell: &Self::Cell = conductor.cell_by_id(cell_id)?;
        cell.invoke_zome(self.clone(), invocation).await.map_err(Into::into)
    }

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
