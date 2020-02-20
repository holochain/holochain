use crate::{conductor::{CellHandle, Conductor}, error::ConductorResult};
use async_trait::async_trait;
use futures::sink::SinkExt;
use mockall::mock;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::{pin::Pin, sync::Arc};
use sx_cell::{
    cell::{autonomic::AutonomicCue, Cell, CellId},
    conductor_api::{ConductorApiError, ConductorCellApiT, ConductorApiResult},
    nucleus::{ZomeInvocation, ZomeInvocationResult},
};
use sx_types::{error::SkunkResult, prelude::*, shims::*, signature::Signature};

#[derive(Clone)]
pub struct ConductorExternalApi<Api: ConductorCellApiT> {
    lock: Arc<RwLock<Conductor<Api>>>,
}

impl<Api: ConductorCellApiT> ConductorExternalApi<Api> {
    pub fn new(lock: Arc<RwLock<Conductor<Api>>>) -> Self {
        Self { lock }
    }
}

impl<Api: ConductorCellApiT> ConductorExternalApi<Api> {
    pub async fn admin(&mut self, _method: AdminMethod) -> ConductorApiResult<JsonString> {
        unimplemented!()
    }

    pub async fn test(
        &mut self,
        _cell: Cell<Api>,
        _invocation: ZomeInvocation,
    ) -> ConductorApiResult<JsonString> {
        unimplemented!()
    }
}


/// It's unsure whether we'll actually use the following

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
