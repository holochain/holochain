use crate::{
    conductor::{CellHandle, Conductor},
    error::ConductorResult,
};
use async_trait::async_trait;
use futures::sink::SinkExt;
use mockall::mock;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::{pin::Pin, sync::Arc};
use sx_core::{
    cell::{autonomic::AutonomicCue, Cell, CellId},
    conductor_api::{ConductorApiError, ConductorApiResult, ConductorCellApiT},
    nucleus::{ZomeInvocation, ZomeInvocationResult},
};
use sx_types::{error::SkunkResult, prelude::*, shims::*, signature::Signature};

#[derive(Clone)]
pub struct ConductorCellApi {
    lock: Arc<RwLock<Conductor<ConductorCellApi>>>,
    cell_id: CellId,
}

impl ConductorCellApi {
    pub fn new(lock: Arc<RwLock<Conductor<ConductorCellApi>>>, cell_id: CellId) -> Self {
        Self { cell_id, lock }
    }
}

#[async_trait(?Send)]
impl ConductorCellApiT for ConductorCellApi {
    async fn invoke_zome(
        &self,
        cell_id: CellId,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResult> {
        let conductor = self.lock.read();
        let cell = conductor
            .cell_by_id(&cell_id)
            .map_err(|e| ConductorApiError::ConductorInceptionError(e.to_string()))?;
        Ok(cell.invoke_zome(self.clone(), invocation).await?)
    }

    async fn network_send(&self, message: Lib3hClientProtocol) -> ConductorApiResult<()> {
        let mut tx = self.lock.read().tx_network().clone();
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
        let conductor = self.lock.write();
        let cell = conductor
            .cell_by_id(&self.cell_id)
            .map_err(|e| ConductorApiError::ConductorInceptionError(e.to_string()))?;
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
