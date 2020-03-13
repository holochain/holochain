use super::error::ConductorApiResult;
use crate::conductor::conductor::Conductor;
use std::sync::Arc;
use sx_types::{
    cell::{CellHandle, CellId},
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
    prelude::*,
    shims::*,
};
use tokio::sync::RwLock;

/// The interface that a Conductor exposes to the outside world.
/// The Conductor lives inside an Arc<RwLock<_>> for the benefit of
/// all other API handles
pub struct ExternalConductorApi {
    conductor_mutex: Arc<RwLock<Conductor>>,
}

impl ExternalConductorApi {
    pub fn new(conductor_mutex: Arc<RwLock<Conductor>>) -> Self {
        Self { conductor_mutex }
    }

    pub async fn invoke_zome(
        &self,
        _cell_id: &CellId,
        _invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse> {
        let _conductor = self.conductor_mutex.read().await;
        unimplemented!()
    }

    pub async fn admin(&mut self, _method: AdminMethod) -> ConductorApiResult<JsonString> {
        unimplemented!()
    }
}

/// The set of messages that a conductor understands how to handle
pub enum ConductorProtocol {
    Admin(Box<AdminMethod>),
    Crypto(Box<Crypto>),
    Network(Box<Lib3hServerProtocol>),
    Test(Box<Test>),
    ZomeInvocation(Box<CellHandle>, Box<ZomeInvocation>),
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

#[allow(dead_code)]
pub struct AddAgentArgs {
    id: String,
    name: String,
}
