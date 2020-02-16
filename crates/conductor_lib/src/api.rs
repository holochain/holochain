use crate::{
    conductor::{CellHandle, Conductor},
    error::{ConductorError, ConductorResult},
};
use async_trait::async_trait;
use futures::sink::SinkExt;
use mockall::mock;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::{pin::Pin, sync::Arc};
use sx_core::{
    cell::{autonomic::AutonomicCue, Cell, CellId},
    nucleus::{ZomeInvocation, ZomeInvocationResult},
};
use sx_types::{error::SkunkResult, prelude::*, shims::*, signature::Signature};

#[async_trait(?Send)]
pub trait ConductorApiInternalT {
    async fn invoke_zome(
        &self,
        cell: Cell,
        invocation: ZomeInvocation,
    ) -> ConductorResult<ZomeInvocationResult>;

    async fn network_send(&self, message: Lib3hClientProtocol) -> ConductorResult<()>;

    async fn network_request(
        &self,
        _message: Lib3hClientProtocol,
    ) -> ConductorResult<Lib3hServerProtocol>;

    async fn autonomic_cue(&self, cue: AutonomicCue) -> ConductorResult<()>;

    async fn crypto_sign(&self, _payload: String) -> ConductorResult<Signature>;

    async fn crypto_encrypt(&self, _payload: String) -> ConductorResult<String>;

    async fn crypto_decrypt(&self, _payload: String) -> ConductorResult<String>;
}

#[derive(Clone)]
pub struct ConductorApiExternal {
    lock: Arc<RwLock<Conductor>>,
}

#[derive(Clone)]
pub struct ConductorApiInternal {
    lock: Arc<RwLock<Conductor>>,
    cell_id: CellId,
}

impl ConductorApiExternal {
    pub fn new(lock: Arc<RwLock<Conductor>>) -> Self {
        Self { lock }
    }
}

impl ConductorApiInternal {
    pub fn new(lock: Arc<RwLock<Conductor>>, cell_id: CellId) -> Self {
        Self { cell_id, lock }
    }
}

#[async_trait(?Send)]
impl ConductorApiInternalT for ConductorApiInternal {
    async fn invoke_zome(
        &self,
        cell: Cell,
        invocation: ZomeInvocation,
    ) -> ConductorResult<ZomeInvocationResult> {
        Ok(cell.invoke_zome(invocation).await?)
    }

    async fn network_send(&self, message: Lib3hClientProtocol) -> ConductorResult<()> {
        let mut tx = self.lock.read().tx_network().clone();
        tx.send(message).await.map_err(|e| e.to_string().into())
    }

    async fn network_request(
        &self,
        _message: Lib3hClientProtocol,
    ) -> ConductorResult<Lib3hServerProtocol> {
        unimplemented!()
    }

    async fn autonomic_cue(&self, cue: AutonomicCue) -> ConductorResult<()> {
        let conductor = self.lock.write();
        let cell = conductor.cell_by_id(&self.cell_id)?;
        let _ = cell.handle_autonomic_process(cue.into()).await;
        Ok(())
    }

    async fn crypto_sign(&self, _payload: String) -> ConductorResult<Signature> {
        unimplemented!()
    }

    async fn crypto_encrypt(&self, _payload: String) -> ConductorResult<String> {
        unimplemented!()
    }

    async fn crypto_decrypt(&self, _payload: String) -> ConductorResult<String> {
        unimplemented!()
    }
}

impl ConductorApiExternal {
    pub async fn admin(&mut self, _method: AdminMethod) -> ConductorResult<JsonString> {
        unimplemented!()
    }

    pub async fn test(
        &mut self,
        _cell: Cell,
        _invocation: ZomeInvocation,
    ) -> ConductorResult<JsonString> {
        unimplemented!()
    }
}

// Unfortunate workaround to get mockall to work with async_trait, due to the complexity of each.
// The mock! expansion here creates mocks on a non-async version of the API, and then the actual trait is implemented
// by delegating each async trait method to its sync counterpart
// See https://github.com/asomers/mockall/issues/75
mock! {
    pub ConductorApiInternal {
        fn sync_invoke_zome(
            &self,
            cell: Cell,
            invocation: ZomeInvocation,
        ) -> ConductorResult<ZomeInvocationResult>;

        fn sync_network_send(&self, message: Lib3hClientProtocol) -> ConductorResult<()>;

        fn sync_network_request(
            &self,
            _message: Lib3hClientProtocol,
        ) -> ConductorResult<Lib3hServerProtocol>;

        fn sync_autonomic_cue(&self, cue: AutonomicCue) -> ConductorResult<()>;

        fn sync_crypto_sign(&self, _payload: String) -> ConductorResult<Signature>;

        fn sync_crypto_encrypt(&self, _payload: String) -> ConductorResult<String>;

        fn sync_crypto_decrypt(&self, _payload: String) -> ConductorResult<String>;
    }
}

#[async_trait(?Send)]
impl ConductorApiInternalT for MockConductorApiInternal {
    async fn invoke_zome(
        &self,
        cell: Cell,
        invocation: ZomeInvocation,
    ) -> ConductorResult<ZomeInvocationResult> {
        self.sync_invoke_zome(cell, invocation)
    }

    async fn network_send(&self, message: Lib3hClientProtocol) -> ConductorResult<()> {
        self.sync_network_send(message)
    }

    async fn network_request(
        &self,
        _message: Lib3hClientProtocol,
    ) -> ConductorResult<Lib3hServerProtocol> {
        self.sync_network_request(_message)
    }

    async fn autonomic_cue(&self, cue: AutonomicCue) -> ConductorResult<()> {
        self.sync_autonomic_cue(cue)
    }

    async fn crypto_sign(&self, _payload: String) -> ConductorResult<Signature> {
        self.sync_crypto_sign(_payload)
    }

    async fn crypto_encrypt(&self, _payload: String) -> ConductorResult<String> {
        self.sync_crypto_encrypt(_payload)
    }

    async fn crypto_decrypt(&self, _payload: String) -> ConductorResult<String> {
        self.sync_crypto_decrypt(_payload)
    }
}


//////////////////////////////////////////////////////////////////////////////////
/// Unused ideas from a while ago
///

/// The set of messages that a conductor understands how to handle
pub enum ConductorProtocol {
    Admin(AdminMethod),
    Crypto(Crypto),
    Network(Lib3hServerProtocol),
    Test(Test),
    ZomeInvocation(CellHandle, ZomeInvocation),
}

pub enum AdminMethod {
    Start(CellHandle),
    Stop(CellHandle),
}

pub enum Crypto {
    Sign(String),
    Decrypt(String),
    Encrypt(String),
}

pub enum Test {
    AddAgent(AddAgentArgs),
}

pub struct AddAgentArgs {
    id: String,
    name: String,
}
