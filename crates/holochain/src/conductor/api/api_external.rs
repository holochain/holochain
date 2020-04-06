use super::error::ConductorApiResult;
use crate::{conductor::conductor::Conductor, core::signal::Signal};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use sx_types::{
    cell::CellHandle,
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
    prelude::*,
};
use tokio::sync::RwLock;

/// The interface that a Conductor exposes to the outside world.
#[async_trait::async_trait]
pub trait ExternalConductorApi: 'static + Send + Sync + Clone {
    /// Invoke a zome function on any cell in this conductor.
    async fn invoke_zome(
        &self,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse>;

    /// Call an admin function to modify this Conductor's behavior
    async fn admin(&self, method: AdminRequest) -> ConductorApiResult<AdminResponse>;

    // -- provided -- //

    async fn handle_request(&self, request: InterfaceMsgIncoming) -> InterfaceMsgOutgoing {
        let res: ConductorApiResult<InterfaceMsgOutgoing> = async move {
            match request {
                InterfaceMsgIncoming::ZomeInvocationRequest(request) => {
                    Ok(InterfaceMsgOutgoing::ZomeInvocationResponse(Box::new(
                        self.invoke_zome(*request).await?,
                    )))
                }
                InterfaceMsgIncoming::AdminRequest(request) => Ok(
                    InterfaceMsgOutgoing::AdminResponse(Box::new(self.admin(*request).await?)),
                ),
                InterfaceMsgIncoming::CryptoRequest(request) => unimplemented!(),
            }
        }
        .await;

        match res {
            Ok(response) => response,
            Err(e) => InterfaceMsgOutgoing::Error(format!("{:?}", e)),
        }
    }
}

/// The Conductor lives inside an Arc<RwLock<_>> which is shared with all
/// other Api references
#[derive(Clone)]
pub struct RealExternalConductorApi {
    conductor_mutex: Arc<RwLock<Conductor>>,
}

impl RealExternalConductorApi {
    /// Create a new instance from a shared Conductor reference
    pub fn new(conductor_mutex: Arc<RwLock<Conductor>>) -> Self {
        Self { conductor_mutex }
    }
}

#[async_trait::async_trait]
impl ExternalConductorApi for RealExternalConductorApi {
    async fn invoke_zome(
        &self,
        _invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse> {
        let _conductor = self.conductor_mutex.read().await;
        unimplemented!()
    }

    async fn admin(&self, _method: AdminRequest) -> ConductorApiResult<AdminResponse> {
        unimplemented!()
    }
}

/// The set of messages that a conductor understands how to respond
// TODO: do we actually want a separate variant for each type of response, or
// just general ones for Signal, Response, and Error?
#[derive(Debug, Serialize, Deserialize, SerializedBytes)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum InterfaceMsgOutgoing {
    Error(String),
    Signal(Box<Signal>),
    AdminResponse(Box<AdminResponse>),
    CryptoResponse(Box<CryptoResponse>),
    ZomeInvocationResponse(Box<ZomeInvocationResponse>),
}

#[allow(missing_docs)]
#[derive(Debug, Serialize, Deserialize)]
pub enum AdminResponse {
    Stub,
}

#[allow(missing_docs)]
#[derive(Debug, Serialize, Deserialize)]
pub enum CryptoResponse {
    Stub,
}

/// The set of messages that a conductor understands how to handle
#[derive(Debug, Serialize, Deserialize, SerializedBytes)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum InterfaceMsgIncoming {
    AdminRequest(Box<AdminRequest>),
    CryptoRequest(Box<CryptoRequest>),
    ZomeInvocationRequest(Box<ZomeInvocation>),
}

#[allow(missing_docs)]
#[derive(Debug, Serialize, Deserialize)]
pub enum AdminRequest {
    Start(CellHandle),
    Stop(CellHandle),
}

#[allow(missing_docs)]
#[derive(Debug, Serialize, Deserialize)]
pub enum CryptoRequest {
    Sign(String),
    Decrypt(String),
    Encrypt(String),
}

#[allow(dead_code, missing_docs)]
#[derive(Debug, Serialize, Deserialize)]
pub struct AddAgentArgs {
    id: String,
    name: String,
}
