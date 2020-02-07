use crate::{
    conductor::{CellHandle, Conductor},
    error::{ConductorError, ConductorResult},
};
use async_trait::async_trait;
use futures::sink::SinkExt;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::sync::Arc;
use sx_core::{
    cell::{autonomic::AutonomicCue, CellApi, CellId},
    nucleus::{ZomeInvocation, ZomeInvocationResult},
};
use sx_types::{error::SkunkResult, prelude::*, shims::*, signature::Signature};

#[derive(Clone)]
pub struct ConductorHandleExternal<Cell: CellApi> {
    lock: Arc<RwLock<Conductor<Cell>>>,
}

#[derive(Clone)]
pub struct ConductorHandleInternal<Cell: CellApi> {
    lock: Arc<RwLock<Conductor<Cell>>>,
    cell_id: CellId,
}

impl<Cell: CellApi> ConductorHandleExternal<Cell> {
    pub fn new(lock: Arc<RwLock<Conductor<Cell>>>) -> Self {
        Self { lock }
    }
}

impl<Cell: CellApi> ConductorHandleInternal<Cell> {
    pub fn new(lock: Arc<RwLock<Conductor<Cell>>>, cell_id: CellId) -> Self {
        Self { cell_id, lock }
    }
}

type ConductorImmutable<'c, Cell> = RwLockReadGuard<'c, Conductor<Cell>>;
type ConductorMutable<'c, Cell> = RwLockWriteGuard<'c, Conductor<Cell>>;

pub trait ConductorApiImmutable<Cell: CellApi>: Send {
    fn conductor(&self) -> ConductorImmutable<Cell>;
}

pub trait ConductorApiMutable<Cell: CellApi>: ConductorApiImmutable<Cell> {
    fn conductor_mut(&mut self) -> ConductorMutable<Cell>;
}

/// An interface for referencing a shared conductor state, used by workflows within a Cell
#[async_trait(?Send)]
pub trait ConductorApiInternal<Cell: CellApi>: ConductorApiImmutable<Cell> {
    async fn invoke_zome(
        &self,
        cell: Cell,
        invocation: ZomeInvocation,
    ) -> ConductorResult<ZomeInvocationResult>;

    async fn network_send(&self, message: Lib3hClientProtocol) -> ConductorResult<()>;

    async fn network_request(
        &self,
        message: Lib3hClientProtocol,
    ) -> ConductorResult<Lib3hServerProtocol>;

    async fn autonomic_cue(&self, cue: AutonomicCue) -> ConductorResult<()>;

    async fn crypto_sign(&self, payload: String) -> ConductorResult<Signature>;
    async fn crypto_encrypt(&self, payload: String) -> ConductorResult<String>;
    async fn crypto_decrypt(&self, payload: String) -> ConductorResult<String>;
}

/// An interface for referencing a shared *mutable* conductor state, used by external sources
/// like interfaces. It may be the case that this is unneeded if we can make the Conductor state completely
/// immutable, meaning we simply throw it away and load a new one whenever we need to change its state
#[async_trait]
pub trait ConductorApiExternal<Cell: CellApi>: ConductorApiMutable<Cell> {
    async fn admin(&mut self, method: AdminMethod) -> ConductorResult<JsonString>;
    async fn test(&mut self, cell: Cell, invocation: ZomeInvocation)
        -> ConductorResult<JsonString>;
}

impl<Cell: CellApi> ConductorApiImmutable<Cell> for ConductorHandleExternal<Cell> {
    fn conductor(&self) -> ConductorImmutable<Cell> {
        self.lock.read()
    }
}

impl<Cell: CellApi> ConductorApiMutable<Cell> for ConductorHandleExternal<Cell> {
    fn conductor_mut(&mut self) -> ConductorMutable<Cell> {
        self.lock.write()
    }
}

impl<Cell: CellApi> ConductorApiImmutable<Cell> for ConductorHandleInternal<Cell> {
    fn conductor(&self) -> ConductorImmutable<Cell> {
        self.lock.read()
    }
}

impl<Cell: CellApi> ConductorApiMutable<Cell> for ConductorHandleInternal<Cell> {
    fn conductor_mut(&mut self) -> ConductorMutable<Cell> {
        self.lock.write()
    }
}

#[async_trait(?Send)]
impl<Cell: CellApi> ConductorApiInternal<Cell> for ConductorHandleInternal<Cell> {
    async fn invoke_zome(
        &self,
        cell: Cell,
        invocation: ZomeInvocation,
    ) -> ConductorResult<ZomeInvocationResult>
    where
        Cell: 'async_trait,
    {
        Ok(cell.invoke_zome(invocation).await?)
    }

    async fn network_send(&self, message: Lib3hClientProtocol) -> ConductorResult<()>
    where
        Cell: 'async_trait,
    {
        let mut tx = self.conductor().tx_network().clone();
        tx.send(message).await.map_err(|e| e.to_string().into())
    }

    async fn network_request(
        &self,
        _message: Lib3hClientProtocol,
    ) -> ConductorResult<Lib3hServerProtocol>
    where
        Cell: 'async_trait,
    {
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

#[async_trait]
impl<Cell: CellApi> ConductorApiExternal<Cell> for ConductorHandleExternal<Cell> {
    async fn admin(&mut self, _method: AdminMethod) -> ConductorResult<JsonString>
    where
        Cell: 'async_trait,
    {
        unimplemented!()
    }

    async fn test(
        &mut self,
        _cell: Cell,
        _invocation: ZomeInvocation,
    ) -> ConductorResult<JsonString>
    where
        Cell: 'async_trait,
    {
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
