use crate::conductor::{CellHandle, Conductor};

use parking_lot::RwLock;
use std::sync::Arc;
use sx_cell::{
    cell::{Cell},
    conductor_api::{ConductorApiResult, ConductorCellApiT},
    nucleus::{ZomeInvocation},
};
use sx_types::{prelude::*, shims::*};

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
