use crate::{
    conductor::{CellHandle, Conductor},
    error::{ConductorError, ConductorResult},
};
use async_trait::async_trait;
use futures::sink::SinkExt;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::sync::Arc;
use sx_core::{
    cell::{autonomic::AutonomicCue, CellId, Cell},
    nucleus::{ZomeInvocation, ZomeInvocationResult},
};
use sx_types::{error::SkunkResult, prelude::*, shims::*, signature::Signature};

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

type ConductorImmutable<'c> = RwLockReadGuard<'c, Conductor>;
type ConductorMutable<'c> = RwLockWriteGuard<'c, Conductor>;

impl ConductorApiInternal {
    pub async fn invoke_zome(
        &self,
        cell: Cell,
        invocation: ZomeInvocation,
    ) -> ConductorResult<ZomeInvocationResult>
    {
        Ok(cell.invoke_zome(invocation).await?)
    }

    pub async fn network_send(&self, message: Lib3hClientProtocol) -> ConductorResult<()>
    {
        let mut tx = self.lock.read().tx_network().clone();
        tx.send(message).await.map_err(|e| e.to_string().into())
    }

    pub async fn network_request(
        &self,
        _message: Lib3hClientProtocol,
    ) -> ConductorResult<Lib3hServerProtocol>
    {
        unimplemented!()
    }

    pub async fn autonomic_cue(&self, cue: AutonomicCue) -> ConductorResult<()> {
        let conductor = self.lock.write();
        let cell = conductor.cell_by_id(&self.cell_id)?;
        let _ = cell.handle_autonomic_process(cue.into()).await;
        Ok(())
    }

    pub async fn crypto_sign(&self, _payload: String) -> ConductorResult<Signature> {
        unimplemented!()
    }

    pub async fn crypto_encrypt(&self, _payload: String) -> ConductorResult<String> {
        unimplemented!()
    }

    pub async fn crypto_decrypt(&self, _payload: String) -> ConductorResult<String> {
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

//////////////////////////////////////////////////////////////////////////////////
///
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
