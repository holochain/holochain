#![deny(missing_docs)]

use super::error::ConductorApiResult;
use crate::conductor::{
    interface::error::{InterfaceError, InterfaceResult},
    ConductorHandle,
};
use holochain_serialized_bytes::prelude::*;
use sx_types::{
    cell::CellHandle,
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
};

/// A trait that unifies both the admin and app interfaces
#[async_trait::async_trait]
pub trait InterfaceApi: 'static + Send + Sync + Clone {
    /// Which request is being made
    type ApiRequest: TryFrom<SerializedBytes, Error = SerializedBytesError> + Send + Sync;
    /// Which response is sent to the above request
    type ApiResponse: TryInto<SerializedBytes, Error = SerializedBytesError> + Send + Sync;
    /// Handle a request on this API
    async fn handle_request(&self, request: Self::ApiRequest)
        -> InterfaceResult<Self::ApiResponse>;
}

/// A trait for the interface that a Conductor exposes to the outside world to use for administering the conductor.  
/// This trait has a one mock implementation and one "Std" implementation
#[async_trait::async_trait]
pub trait AdminInterfaceApi: 'static + Send + Sync + Clone {
    /// Call an admin function to modify this Conductor's behavior
    async fn admin(&self, method: AdminRequest) -> ConductorApiResult<AdminResponse>;

    // -- provided -- //

    /// Route the request to be handled
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

    /// Routes the [AppRequest] to the [AppResponse]
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

/// The admin interface that external conections
/// can use to make requests to the conductor
#[derive(Clone)]
pub struct StdAdminInterfaceApi {
    conductor_handle: ConductorHandle,
    app_api: StdAppInterfaceApi,
}

impl StdAdminInterfaceApi {
    pub(crate) fn new(conductor_handle: ConductorHandle) -> Self {
        let app_api = StdAppInterfaceApi::new(conductor_handle.clone());
        StdAdminInterfaceApi {
            conductor_handle,
            app_api,
        }
    }
}

#[async_trait::async_trait]
impl AdminInterfaceApi for StdAdminInterfaceApi {
    async fn admin(&self, request: AdminRequest) -> ConductorApiResult<AdminResponse> {
        Ok(AdminResponse::Unimplemented(request))
        // use AdminRequest::*;
        // match request {
        //     Start(cell_handle) => unimplemented!(),
        //     Stop(cell_handle) => unimplemented!(),
        //     AddDna => unimplemented!(),
        // }
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
        self.conductor_handle
            .read()
            .await
            // Make sure the conductor is not in the process of shutting down
            .check_running()
            .map_err(InterfaceError::RequestHandler)?;
        let r = AdminInterfaceApi::handle_request(self, request).await;
        Ok(r)
    }
}

/// The Conductor lives inside an Arc<RwLock<_>> which is shared with all
/// other Api references
#[derive(Clone)]
pub struct StdAppInterfaceApi {
    conductor_handle: ConductorHandle,
}

impl StdAppInterfaceApi {
    /// Create a new instance from a shared Conductor reference
    pub fn new(conductor_handle: ConductorHandle) -> Self {
        Self { conductor_handle }
    }
}

#[async_trait::async_trait]
impl AppInterfaceApi for StdAppInterfaceApi {
    async fn invoke_zome(
        &self,
        _invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse> {
        let _conductor = self.conductor_handle.read().await;
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
        self.conductor_handle
            .read()
            .await
            .check_running()
            .map_err(InterfaceError::RequestHandler)?;
        let r = AppInterfaceApi::handle_request(self, request).await;
        Ok(r)
    }
}
/// The set of messages that a conductor understands how to respond
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(tag = "type")]
pub enum AppResponse {
    /// There has been an error in the request
    Error {
        // TODO maybe this could be serialized instead of stringified?
        /// Stringified version of the error
        debug: String,
    },
    /// The response to a zome call
    ZomeInvocationResponse {
        /// The data that was returned by this call
        response: Box<ZomeInvocationResponse>,
    },
}

#[allow(missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub enum AdminResponse {
    Unimplemented(AdminRequest),
    DnaAdded,
    Error { debug: String },
}

/// The set of messages that a conductor understands how to handle over an App interface
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(tag = "type")]
pub enum AppRequest {
    /// TODO: DOCS: what is this?
    CryptoRequest {
        /// TODO: DOCS: ?
        request: Box<CryptoRequest>,
    },
    /// Request used for testing
    TestRequest {
        /// Tets request data
        request: Box<TestRequest>,
    },
    /// Call a zome function
    ZomeInvocationRequest {
        /// Information about which zome call you want to make
        request: Box<ZomeInvocation>,
    },
}

/// The set of messages that a conductor understands how to handle over an Admin interface
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
