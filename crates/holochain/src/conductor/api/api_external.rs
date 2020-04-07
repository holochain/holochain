use super::error::ConductorApiResult;
use crate::conductor::conductor::Conductor;
use std::sync::Arc;
use sx_types::{
    cell::CellHandle,
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
};
use tokio::sync::RwLock;

/// The interface that a Conductor exposes to the outside world.
#[async_trait::async_trait]
pub trait AdminConductorApi: 'static + Send + Sync + Clone {
    /// Call an admin function to modify this Conductor's behavior
    async fn admin(&self, method: AdminRequest) -> ConductorApiResult<AdminResponse>;

    // -- provided -- //

    async fn handle_request(&self, request: ConductorRequest) -> ConductorResponse {
        let res: ConductorApiResult<ConductorResponse> = async move {
            match request {
                ConductorRequest::AdminRequest { request } => {
                    Ok(ConductorResponse::AdminResponse {
                        response: Box::new(self.admin(*request).await?),
                    })
                }
                _ => unimplemented!(),
            }
        }
        .await;

        match res {
            Ok(response) => response,
            Err(e) => ConductorResponse::Error {
                debug: format!("{:?}", e),
            },
        }
    }
}
/// The interface that a Conductor exposes to the outside world.
#[async_trait::async_trait]
pub trait ExternalConductorApi: 'static + Send + Sync + Clone {
    /// Invoke a zome function on any cell in this conductor.
    async fn invoke_zome(
        &self,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse>;

    // -- provided -- //

    async fn handle_request(&self, request: ConductorRequest) -> ConductorResponse {
        let res: ConductorApiResult<ConductorResponse> = async move {
            match request {
                ConductorRequest::ZomeInvocationRequest { request } => {
                    Ok(ConductorResponse::ZomeInvocationResponse {
                        response: Box::new(self.invoke_zome(*request).await?),
                    })
                }
                _ => unimplemented!(),
            }
        }
        .await;

        match res {
            Ok(response) => response,
            Err(e) => ConductorResponse::Error {
                debug: format!("{:?}", e),
            },
        }
    }
}

/// The Conductor lives inside an Arc<RwLock<_>> which is shared with all
/// other Api references
#[derive(Clone)]
pub struct StdExternalConductorApi {
    conductor_mutex: Arc<RwLock<Conductor>>,
}

impl StdExternalConductorApi {
    /// Create a new instance from a shared Conductor reference
    pub fn new(conductor_mutex: Arc<RwLock<Conductor>>) -> Self {
        Self { conductor_mutex }
    }
}

#[async_trait::async_trait]
impl ExternalConductorApi for StdExternalConductorApi {
    async fn invoke_zome(
        &self,
        _invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse> {
        let _conductor = self.conductor_mutex.read().await;
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
    AdminRequest { request: Box<AdminRequest> },
    CryptoRequest { request: Box<CryptoRequest> },
    TestRequest { request: Box<TestRequest> },
    ZomeInvocationRequest { request: Box<ZomeInvocation> },
}
holochain_serialized_bytes::holochain_serial!(ConductorRequest);

#[allow(missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum AdminRequest {
    Start(CellHandle),
    Stop(CellHandle),
}

#[allow(missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum CryptoRequest {
    Sign(String),
    Decrypt(String),
    Encrypt(String),
}

#[allow(missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum TestRequest {
    AddAgent(AddAgentArgs),
}

#[allow(dead_code, missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AddAgentArgs {
    id: String,
    name: String,
}
