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
/// The Conductor lives inside an Arc<RwLock<_>> which is shared with all
/// other Api references
pub struct ExternalConductorApi {
    conductor_mutex: Arc<RwLock<Conductor>>,
}

impl ExternalConductorApi {
    /// Create a new instance from a shared Conductor reference
    pub fn new(conductor_mutex: Arc<RwLock<Conductor>>) -> Self {
        Self { conductor_mutex }
    }

    /// Invoke a zome function on any cell in this conductor.
    pub async fn invoke_zome(
        &self,
        _cell_id: &CellId,
        _invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse> {
        let _conductor = self.conductor_mutex.read().await;
        unimplemented!()
    }

    /// Call an admin function to modify this Conductor's behavior
    pub async fn admin(&mut self, _method: AdminMethod) -> ConductorApiResult<JsonString> {
        unimplemented!()
    }
}

#[allow(missing_docs)]
/// The set of messages that a conductor understands how to handle
pub enum ConductorProtocol {
    Admin(Box<AdminMethod>),
    Crypto(Box<Crypto>),
    Network(Box<Lib3hServerProtocol>),
    Test(Box<Test>),
    ZomeInvocation(Box<CellHandle>, Box<ZomeInvocation>),
}

#[allow(missing_docs)]
pub enum AdminMethod {
    Start(CellHandle),
    Stop(CellHandle),
}

#[allow(missing_docs)]
pub enum Crypto {
    Sign(String),
    Decrypt(String),
    Encrypt(String),
}

#[allow(missing_docs)]
pub enum Test {
    AddAgent(AddAgentArgs),
}

#[allow(dead_code, missing_docs)]
pub struct AddAgentArgs {
    id: String,
    name: String,
}
