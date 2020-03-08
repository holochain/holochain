use crate::conductor::Conductor;
use async_trait::async_trait;
use futures::sink::SinkExt;
use shrinkwraprs::Shrinkwrap;
use std::sync::Arc;
use sx_cell::cell::CellId;
use sx_conductor_api::{
    CellConductorInterfaceT, CellT, ConductorApiError, ConductorApiResult, ConductorT,
};
use sx_types::{
    autonomic::AutonomicCue,
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
    shims::*,
    signature::Signature,
};
use tokio::sync::{RwLock, RwLockReadGuard};

/// The concrete implementation of [CellConductorInterfaceT], which is used to give
/// Cells an API for calling back to their [Conductor].
#[derive(Clone)]
pub struct CellConductorInterface {
    lock: Arc<RwLock<Conductor>>,
    cell_id: CellId,
}

impl CellConductorInterface {
    pub fn new(lock: Arc<RwLock<Conductor>>, cell_id: CellId) -> Self {
        Self { cell_id, lock }
    }
}

#[async_trait(?Send)]
impl CellConductorInterfaceT for CellConductorInterface {
    type Cell = Cell<Self>;
    type Conductor = Conductor<Self>;

    async fn conductor_ref(&self) -> RwLockReadGuard<'_, Self::Conductor> {
        self.lock.read().await
    }

    // async fn invoke_zome(
    //     &self,
    //     cell_id: &CellId,
    //     invocation: ZomeInvocation,
    // ) -> ConductorApiResult<ZomeInvocationResponse> {
    //     let conductor = self.lock.read();
    //     let cell = conductor.cell_by_id(&cell_id)?;
    //     Ok(cell.invoke_zome(self.clone(), invocation).await?)
    // }

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
/// needs to know the concrete [CellConductorInterfaceT] implementation, which is also
/// defined in this crate. The Conductor and everything in this crate should refer
/// to this wrapper type, to make the [CellConductorInterface] types work out.
#[derive(Shrinkwrap)]
pub struct Cell<I = CellConductorInterface>(
    #[shrinkwrap(main_field)] sx_cell::cell::Cell,
    std::marker::PhantomData<I>,
);

#[async_trait]
impl<I: CellConductorInterfaceT> CellT for Cell<I> {
    type Interface = I;

    async fn invoke_zome(
        &self,
        _conductor_api: Self::Interface,
        _invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse> {
        unimplemented!()
    }
}
