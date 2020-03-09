use crate::error::ConductorApiResult;
use async_trait::async_trait;
use sx_types::{
    cell::{CellHandle, CellId},
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
    prelude::JsonString,
    shims::*,
};

/// The "external" Conductor API, which is used by e.g. Interfaces
/// to control a [Conductor] externally
#[async_trait]
pub trait ExternalConductorApiT: Send + Sync + Sized {
    async fn admin(&mut self, _method: AdminMethod) -> ConductorApiResult<JsonString> {
        unimplemented!()
    }

    async fn invoke_zome(
        &self,
        cell_id: &CellId,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse>;
}

// It's uncertain whether we'll actually use all of the following

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

#[allow(dead_code)]
pub struct AddAgentArgs {
    id: String,
    name: String,
}
