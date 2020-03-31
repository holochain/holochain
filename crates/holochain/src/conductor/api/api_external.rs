use super::error::ConductorApiResult;
use crate::conductor::conductor::Conductor;
use std::sync::Arc;
use sx_types::{
    cell::CellHandle,
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
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

    pub async fn handle_request(&self, request: ConductorRequest) -> ConductorResponse {
        match self.handle_request_inner(request).await {
            Ok(response) => response,
            Err(e) => ConductorResponse::Error {
                debug: format!("{:?}", e),
            },
        }
    }

    async fn handle_request_inner(
        &self,
        request: ConductorRequest,
    ) -> ConductorApiResult<ConductorResponse> {
        match request {
            ConductorRequest::ZomeInvocation { cell, request } => {
                Ok(ConductorResponse::ZomeInvocationResponse {
                    response: Box::new(self.invoke_zome(&cell, *request).await?),
                })
            }
            ConductorRequest::Admin { request } => Ok(ConductorResponse::AdminResponse {
                response: Box::new(self.admin(*request).await?),
            }),
            _ => unimplemented!(),
        }
    }

    /// Invoke a zome function on any cell in this conductor.
    async fn invoke_zome(
        &self,
        _cell_handle: &CellHandle,
        _invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse> {
        let _conductor = self.conductor_mutex.read().await;
        unimplemented!()
    }

    /// Call an admin function to modify this Conductor's behavior
    async fn admin(&self, _method: AdminMethod) -> ConductorApiResult<AdminResponse> {
        unimplemented!()
    }
}

/// The set of messages that a conductor understands how to respond
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ConductorResponse {
    Error {
        debug: String,
    },
    AdminResponse {
        response: Box<AdminResponse>,
    },
    ZomeInvocationResponse {
        response: Box<ZomeInvocationResponse>,
    },
}
holochain_serialized_bytes::holochain_serial!(ConductorResponse);

#[allow(missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum AdminResponse {
    Stub,
}

/// The set of messages that a conductor understands how to handle
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ConductorRequest {
    Admin {
        request: Box<AdminMethod>,
    },
    Crypto {
        request: Box<Crypto>,
    },
    Test {
        request: Box<Test>,
    },
    ZomeInvocation {
        cell: Box<CellHandle>,
        request: Box<ZomeInvocation>,
    },
}
holochain_serialized_bytes::holochain_serial!(ConductorRequest);

#[allow(missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum AdminMethod {
    Start(CellHandle),
    Stop(CellHandle),
}

#[allow(missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum Crypto {
    Sign(String),
    Decrypt(String),
    Encrypt(String),
}

#[allow(missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum Test {
    AddAgent(AddAgentArgs),
}

#[allow(dead_code, missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AddAgentArgs {
    id: String,
    name: String,
}
