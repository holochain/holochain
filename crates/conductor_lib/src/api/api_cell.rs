use crate::conductor::Conductor;
use async_trait::async_trait;
use shrinkwraprs::Shrinkwrap;
use std::sync::Arc;
use sx_cell::cell::CellId;
use sx_conductor_api::{
    CellConductorApiT, CellT, ConductorApiError, ConductorApiResult, ConductorT,
};
use sx_types::{
    autonomic::AutonomicCue,
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
}

/// A wrapper around the actual [Cell] implementation, which is only necessary because
/// we need to implement [CellT] in this crate, not the sx_cell crate, because [CellT]
/// needs to know the concrete [CellConductorApiT] implementation, which is also
/// defined in this crate. The Conductor and everything in this crate should refer
/// to this wrapper type, to make the [CellConductorApi] types work out.
#[derive(Shrinkwrap)]
pub struct Cell<I = CellConductorApi>(
    #[shrinkwrap(main_field)] sx_cell::cell::Cell,
    std::marker::PhantomData<I>,
);

/// This is weird. Because CellT needs to know the concrete [CellConductorApiT],
/// we need to implement it here, and not in the sx_cell crate. This is strongly
/// pointing towards a restructuring, where the Cell becomes a conductor-specific
/// concept, and the "cell" crate becomes more geared towards the Workflows, which
/// just get resources passed in and perform some work, without the notion of being
/// "a Cell"
#[async_trait]
impl<I: CellConductorApiT> CellT for Cell<I> {
    type Api = I;

    async fn invoke_zome(
        &self,
        _conductor_api: Self::Api,
        _invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse> {
        unimplemented!()
    }

    // TODO: if things stay this way, as mentioned in the comment for this impl,
    // then all other implementations for the important Cell methods would go here
}
