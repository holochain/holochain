use super::{InterfaceApi, SignalSubscription};
use crate::conductor::{
    api::error::{ConductorApiResult, ExternalApiWireError, SerializationError},
    state::AppInterfaceId,
};
use crate::conductor::{
    interface::error::{InterfaceError, InterfaceResult},
    ConductorHandle,
};
use crate::core::ribosome::ZomeCallInvocation;
use holochain_serialized_bytes::prelude::*;
use holochain_types::app::{AppId, InstalledApp};
use holochain_zome_types::ExternOutput;
use holochain_zome_types::ZomeCallResponse;

/// The interface that a Conductor exposes to the outside world.
#[async_trait::async_trait]
pub trait AppInterfaceApi: 'static + Send + Sync + Clone {
    /// Call an admin function to modify this Conductor's behavior
    async fn handle_app_request_inner(
        &self,
        request: AppRequest,
    ) -> ConductorApiResult<AppResponse>;

    // -- provided -- //

    /// Deal with error cases produced by `handle_app_request_inner`
    async fn handle_app_request(&self, request: AppRequest) -> AppResponse {
        let res = self.handle_app_request_inner(request).await;

        match res {
            Ok(response) => response,
            Err(e) => AppResponse::Error(e.into()),
        }
    }
}

/// The Conductor lives inside an Arc<RwLock<_>> which is shared with all
/// other Api references
#[derive(Clone)]
pub struct RealAppInterfaceApi {
    conductor_handle: ConductorHandle,
    interface_id: AppInterfaceId,
}

impl RealAppInterfaceApi {
    /// Create a new instance from a shared Conductor reference
    pub fn new(conductor_handle: ConductorHandle, interface_id: AppInterfaceId) -> Self {
        Self {
            conductor_handle,
            interface_id,
        }
    }
}

#[async_trait::async_trait]
impl AppInterfaceApi for RealAppInterfaceApi {
    /// Routes the [AppRequest] to the [AppResponse]
    async fn handle_app_request_inner(
        &self,
        request: AppRequest,
    ) -> ConductorApiResult<AppResponse> {
        match request {
            AppRequest::AppInfo { app_id } => Ok(AppResponse::AppInfo(
                self.conductor_handle.get_app_info(&app_id).await?,
            )),
            AppRequest::SignalSubscription(_subscription) => {
                todo!("Signal pubsub not yet implemented")
            }
            AppRequest::ZomeCallInvocation(request) => {
                match self.conductor_handle.call_zome(*request).await? {
                    Ok(ZomeCallResponse::Ok(output)) => {
                        Ok(AppResponse::ZomeCallInvocation(Box::new(output)))
                    }
                    Ok(ZomeCallResponse::Unauthorized) => Ok(AppResponse::ZomeCallUnauthorized),
                    Err(e) => Ok(AppResponse::Error(e.into())),
                }
            }
            AppRequest::Crypto(_) => unimplemented!("Crypto methods currently unimplemented"),
        }
    }
}

#[async_trait::async_trait]
impl InterfaceApi for RealAppInterfaceApi {
    type ApiRequest = AppRequest;
    type ApiResponse = AppResponse;
    async fn handle_request(
        &self,
        request: Result<Self::ApiRequest, SerializedBytesError>,
    ) -> InterfaceResult<Self::ApiResponse> {
        {
            self.conductor_handle
                .check_running()
                .await
                .map_err(Box::new)
                .map_err(InterfaceError::RequestHandler)?;
        }
        match request {
            Ok(request) => Ok(AppInterfaceApi::handle_app_request(self, request).await),
            Err(e) => Ok(AppResponse::Error(SerializationError::from(e).into())),
        }
    }
}

/// The set of messages that a conductor understands how to handle over an App interface
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum AppRequest {
    /// Get info about the App
    AppInfo {
        /// The AppId for which to get information
        app_id: AppId,
    },

    /// Asks the conductor to do some crypto
    Crypto(Box<CryptoRequest>),

    /// Call a zome function
    ZomeCallInvocation(Box<ZomeCallInvocation>),

    /// Update signal subscriptions
    SignalSubscription(SignalSubscription),
}

/// Responses to requests received on an App interface
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum AppResponse {
    /// There has been an error in the request
    Error(ExternalApiWireError),

    /// The response to an AppInfo request
    AppInfo(Option<InstalledApp>),

    /// The response to a zome call
    ZomeCallInvocation(Box<ExternOutput>),

    /// The response to a SignalSubscription message
    SignalSubscriptionUpdated,

    /// The zome call is unauthorized
    // TODO: I think this should be folded into ExternalApiWireError -MD
    ZomeCallUnauthorized,
}

#[allow(missing_docs)]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum CryptoRequest {
    Sign(String),
    Decrypt(String),
    Encrypt(String),
}
