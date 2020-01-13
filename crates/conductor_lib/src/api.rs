use crate::conductor::CellHandle;
use crate::conductor::Conductor;
use async_trait::async_trait;
use crossbeam_channel::Sender;
use holochain_json_api::json::JsonString;
use lib3h_protocol::protocol_client::Lib3hClientProtocol;
use lib3h_protocol::protocol_server::Lib3hServerProtocol;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use skunkworx_core::cell::Cell;
use skunkworx_core::cell::CellApi;
use skunkworx_core::types::ZomeInvocation;
use skunkworx_core::types::ZomeInvocationResult;
use skunkworx_core_types::error::SkunkResult;
use std::sync::Arc;

#[derive(Clone)]
struct ConductorHandle<Cell: CellApi>(Arc<RwLock<Conductor<Cell>>>);

type ConductorImmutable<'c, Cell> = RwLockReadGuard<'c, Conductor<Cell>>;
type ConductorMutable<'c, Cell> = RwLockWriteGuard<'c, Conductor<Cell>>;

pub trait ConductorApiImmutable<Cell: CellApi> {
    fn conductor(&self) -> ConductorImmutable<Cell>;
}

pub trait ConductorApiMutable<Cell: CellApi>: ConductorApiImmutable<Cell> {
    fn conductor_mut(&mut self) -> ConductorMutable<Cell>;
}

#[async_trait]
pub trait ConductorApiInternal<Cell: CellApi>: ConductorApiImmutable<Cell> {
    async fn invoke_zome(&self, cell: Cell, invocation: ZomeInvocation) -> ZomeInvocationResult;
    async fn net_send(&self, message: Lib3hClientProtocol) -> SkunkResult<()>;
    async fn net_request(&self, message: Lib3hClientProtocol) -> SkunkResult<Lib3hServerProtocol>;
}

#[async_trait]
pub trait ConductorApiExternal<Cell: CellApi>: ConductorApiMutable<Cell> {
    async fn admin(&mut self, method: AdminMethod) -> SkunkResult<JsonString>;
    async fn test(&mut self, cell: Cell, invocation: ZomeInvocation) -> ZomeInvocationResult;
}

impl<Cell: CellApi> ConductorApiImmutable<Cell> for ConductorHandle<Cell> {
    fn conductor(&self) -> ConductorImmutable<Cell> {
        self.0.read()
    }
}

impl<Cell: CellApi> ConductorApiMutable<Cell> for ConductorHandle<Cell> {
    fn conductor_mut(&mut self) -> ConductorMutable<Cell> {
        self.0.write()
    }
}

#[async_trait]
impl<Cell: CellApi> ConductorApiInternal<Cell> for ConductorHandle<Cell> {
    async fn invoke_zome(&self, cell: Cell, invocation: ZomeInvocation) -> ZomeInvocationResult
    where
        Cell: 'async_trait,
    {
        self.conductor().blah();
        unimplemented!()
    }

    async fn net_send(&self, message: Lib3hClientProtocol) -> SkunkResult<()>
    where
        Cell: 'async_trait,
    {
        unimplemented!()
    }

    async fn net_request(&self, message: Lib3hClientProtocol) -> SkunkResult<Lib3hServerProtocol>
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

    async fn test(&mut self, cell: Cell, invocation: ZomeInvocation) -> ZomeInvocationResult
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

pub enum AdminMethod {}

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
