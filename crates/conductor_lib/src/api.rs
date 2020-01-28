use crate::conductor::CellHandle;
use crate::conductor::Conductor;
use async_trait::async_trait;
use futures::sink::SinkExt;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::sync::Arc;
use sx_core::cell::CellApi;
use sx_core::types::ZomeInvocation;
use sx_core::types::ZomeInvocationResult;
use sx_types::error::SkunkResult;
use sx_types::shims::*;
use sx_types::JsonString;

#[derive(Clone)]
pub struct ConductorHandle<Cell: CellApi> {
    lock: Arc<RwLock<Conductor<Cell>>>,
}

impl<Cell: CellApi> ConductorHandle<Cell> {
    pub fn new(lock: Arc<RwLock<Conductor<Cell>>>) -> Self {
        Self { lock }
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
#[async_trait]
pub trait ConductorApiInternal<Cell: CellApi>: ConductorApiImmutable<Cell> {
    async fn invoke_zome(
        &self,
        cell: Cell,
        invocation: ZomeInvocation,
    ) -> SkunkResult<ZomeInvocationResult>;
    async fn network_send(&self, message: Lib3hClientProtocol) -> SkunkResult<()>;
    async fn network_request(
        &self,
        message: Lib3hClientProtocol,
    ) -> SkunkResult<Lib3hServerProtocol>;
}

/// An interface for referencing a shared *mutable* conductor state, used by external sources
/// like interfaces. It may be the case that this is unneeded if we can make the Conductor state completely
/// immutable, meaning we simply throw it away and load a new one whenever we need to change its state
#[async_trait]
pub trait ConductorApiExternal<Cell: CellApi>: ConductorApiMutable<Cell> {
    async fn admin(&mut self, method: AdminMethod) -> SkunkResult<JsonString>;
    async fn test(&mut self, cell: Cell, invocation: ZomeInvocation) -> SkunkResult<JsonString>;
}

impl<Cell: CellApi> ConductorApiImmutable<Cell> for ConductorHandle<Cell> {
    fn conductor(&self) -> ConductorImmutable<Cell> {
        self.lock.read()
    }
}

impl<Cell: CellApi> ConductorApiMutable<Cell> for ConductorHandle<Cell> {
    fn conductor_mut(&mut self) -> ConductorMutable<Cell> {
        self.lock.write()
    }
}

#[async_trait]
impl<Cell: CellApi> ConductorApiInternal<Cell> for ConductorHandle<Cell> {
    async fn invoke_zome(
        &self,
        cell: Cell,
        invocation: ZomeInvocation,
    ) -> SkunkResult<ZomeInvocationResult>
    where
        Cell: 'async_trait,
    {
        cell.invoke_zome(invocation).await
    }

    async fn network_send(&self, message: Lib3hClientProtocol) -> SkunkResult<()>
    where
        Cell: 'async_trait,
    {
        let mut tx = self.conductor().tx_network().clone();
        tx.send(message).await.map_err(|e| e.to_string().into())
    }

    async fn network_request(
        &self,
        message: Lib3hClientProtocol,
    ) -> SkunkResult<Lib3hServerProtocol>
    where
        Cell: 'async_trait,
    {
        unimplemented!()
    }
}

#[async_trait]
impl<Cell: CellApi> ConductorApiExternal<Cell> for ConductorHandle<Cell> {
    async fn admin(&mut self, method: AdminMethod) -> SkunkResult<JsonString>
    where
        Cell: 'async_trait,
    {
        unimplemented!()
    }

    async fn test(&mut self, cell: Cell, invocation: ZomeInvocation) -> SkunkResult<JsonString>
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
