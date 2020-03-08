use sx_types::agent::CellHandle;
use sx_types::prelude::JsonString;
use sx_types::signature::Signature;
use sx_types::autonomic::AutonomicCue;
use sx_types::shims::*;
use sx_types::nucleus::ZomeInvocation;
use sx_types::nucleus::ZomeInvocationResponse;
use crate::error::ConductorApiResult;
use crate::cell::CellT;
use sx_types::agent::CellId;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::sync::Arc;
use crate::conductor::ConductorT;

use async_trait::async_trait;

/// The interface for a Cell to talk to its calling Conductor
#[async_trait]
pub trait ExternalConductorInterfaceT: Send + Sync + Sized
{
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
