use super::error::ConductorApiResult;
use crate::conductor::{conductor::Conductor, interface::error::InterfaceResult};
use holochain_serialized_bytes::prelude::*;
use std::sync::Arc;
use sx_types::{
    cell::CellHandle,
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
};
use tokio::sync::RwLock;

#[async_trait::async_trait]
pub trait InterfaceApi: 'static + Send + Sync + Clone {
    type ApiRequest: TryFrom<SerializedBytes, Error = SerializedBytesError> + Send + Sync;
    type ApiResponse: TryInto<SerializedBytes, Error = SerializedBytesError> + Send + Sync;
    async fn handle_request(&self, request: Self::ApiRequest)
        -> InterfaceResult<Self::ApiResponse>;
}

/// The interface that a Conductor exposes to the outside world.
#[async_trait::async_trait]
pub trait AdminInterfaceApi: 'static + Send + Sync + Clone {
    /// Call an admin function to modify this Conductor's behavior
    async fn admin(&self, method: AdminRequest) -> ConductorApiResult<AdminResponse>;

    // -- provided -- //

    async fn handle_request(&self, request: AdminRequest) -> AdminResponse {
        let res = self.admin(request).await;

        match res {
            Ok(response) => response,
            Err(e) => AdminResponse::Error {
                debug: format!("{:?}", e),
            },
        }
    }
}
/// The interface that a Conductor exposes to the outside world.
#[async_trait::async_trait]
pub trait AppInterfaceApi: 'static + Send + Sync + Clone {
    /// Invoke a zome function on any cell in this conductor.
    async fn invoke_zome(
        &self,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse>;

    // -- provided -- //

    async fn handle_request(&self, request: AppRequest) -> AppResponse {
        let res: ConductorApiResult<AppResponse> = async move {
            match request {
                AppRequest::ZomeInvocationRequest { request } => {
                    Ok(AppResponse::ZomeInvocationResponse {
                        response: Box::new(self.invoke_zome(*request).await?),
                    })
                }
                _ => unimplemented!(),
            }
        }
        .await;

        match res {
            Ok(response) => response,
            Err(e) => AppResponse::Error {
                debug: format!("{:?}", e),
            },
        }
    }
}

#[derive(Clone)]
pub struct StdAdminInterfaceApi {
    conductor_mutex: Arc<RwLock<Conductor>>,
}

impl StdAdminInterfaceApi {
    pub(crate) fn new(conductor_mutex: Arc<RwLock<Conductor>>) -> Self {
        StdAdminInterfaceApi { conductor_mutex }
    }
}

#[async_trait::async_trait]
impl AdminInterfaceApi for StdAdminInterfaceApi {
    async fn admin(&self, _method: AdminRequest) -> ConductorApiResult<AdminResponse> {
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl InterfaceApi for StdAdminInterfaceApi {
    type ApiRequest = AdminRequest;
    type ApiResponse = AdminResponse;
    async fn handle_request(
        &self,
        request: Self::ApiRequest,
    ) -> InterfaceResult<Self::ApiResponse> {
        let r = AdminInterfaceApi::handle_request(self, request).await;
        Ok(r)
    }
}

/// The Conductor lives inside an Arc<RwLock<_>> which is shared with all
/// other Api references
#[derive(Clone)]
pub struct StdAppInterfaceApi {
    conductor_mutex: Arc<RwLock<Conductor>>,
}

impl StdAppInterfaceApi {
    /// Create a new instance from a shared Conductor reference
    pub fn new(conductor_mutex: Arc<RwLock<Conductor>>) -> Self {
        Self { conductor_mutex }
    }
}

#[async_trait::async_trait]
impl AppInterfaceApi for StdAppInterfaceApi {
    async fn invoke_zome(
        &self,
        _invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse> {
        let _conductor = self.conductor_mutex.read().await;
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl InterfaceApi for StdAppInterfaceApi {
    type ApiRequest = AppRequest;
    type ApiResponse = AppResponse;
    async fn handle_request(
        &self,
        request: Self::ApiRequest,
    ) -> InterfaceResult<Self::ApiResponse> {
        let r = AppInterfaceApi::handle_request(self, request).await;
        Ok(r)
    }
}
/// The set of messages that a conductor understands how to respond
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(tag = "type")]
pub enum AppResponse {
    Error {
        debug: String,
    },
    ZomeInvocationResponse {
        response: Box<ZomeInvocationResponse>,
    },
}

#[allow(missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub enum AdminResponse {
    Stub,
    DnaAdded,
    Error { debug: String },
}

/// The set of messages that a conductor understands how to handle
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(tag = "type")]
pub enum AppRequest {
    CryptoRequest { request: Box<CryptoRequest> },
    TestRequest { request: Box<TestRequest> },
    ZomeInvocationRequest { request: Box<ZomeInvocation> },
}

#[allow(missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub enum AdminRequest {
    Start(CellHandle),
    Stop(CellHandle),
    AddDna,
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
