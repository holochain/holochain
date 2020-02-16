use crate::{conductor::{CellHandle, Conductor}, error::ConductorResult};
use async_trait::async_trait;
use futures::sink::SinkExt;
use mockall::mock;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::{pin::Pin, sync::Arc};
use sx_core::{
    cell::{autonomic::AutonomicCue, Cell, CellId},
    conductor_api::{ConductorApiError, ConductorCellApiT, ConductorApiResult},
    nucleus::{ZomeInvocation, ZomeInvocationResult},
};
use sx_types::{error::SkunkResult, prelude::*, shims::*, signature::Signature};

#[derive(Clone)]
pub struct ConductorCellApi {
    lock: Arc<RwLock<Conductor>>,
    cell_id: CellId,
}

impl ConductorCellApi {
    pub fn new(lock: Arc<RwLock<Conductor>>, cell_id: CellId) -> Self {
        Self { cell_id, lock }
    }
}


#[async_trait(?Send)]
impl ConductorCellApiT for ConductorCellApi {
    async fn invoke_zome(
        &self,
        cell: Cell,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResult> {
        Ok(cell.invoke_zome(invocation).await?)
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


// Unfortunate workaround to get mockall to work with async_trait, due to the complexity of each.
// The mock! expansion here creates mocks on a non-async version of the API, and then the actual trait is implemented
// by delegating each async trait method to its sync counterpart
// See https://github.com/asomers/mockall/issues/75
mock! {
    pub ConductorCellApi {
        fn sync_invoke_zome(
            &self,
            cell: Cell,
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
        cell: Cell,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResult> {
        self.sync_invoke_zome(cell, invocation)
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
