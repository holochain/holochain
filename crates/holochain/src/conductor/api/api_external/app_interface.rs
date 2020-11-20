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
use holochain_types::app::{InstalledApp, InstalledAppId};
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
            AppRequest::AppInfo { installed_app_id } => Ok(AppResponse::AppInfo(
                self.conductor_handle
                    .get_app_info(&installed_app_id)
                    .await?,
            )),
            AppRequest::ZomeCallInvocation(request) => {
                let req = request.clone();
                match self.conductor_handle.call_zome(*request).await? {
                Ok(ZomeCallResponse::Ok(output)) => {
                  Ok(AppResponse::ZomeCallInvocation(Box::new(output)))
                }
                Ok(ZomeCallResponse::Unauthorized) => {
                  Ok(AppResponse::Error(
                    ExternalApiWireError::ZomeCallUnauthorized(
                      format!("No capabilities grant has been committed that allows the CapSecret {:?} to call the function {} in zome {}", req.cap, req.fn_name, req.zome_name)
                    )
                  ))
                },
                Ok(ZomeCallResponse::NetworkError(e)) => unreachable!("Interface zome calls should never be routed to the network. This is a bug. Got {}", e),
                Err(e) => Ok(AppResponse::Error(e.into())),
              }
            }
            AppRequest::SignalSubscription(_) => Ok(AppResponse::Unimplemented(request)),
            AppRequest::Crypto(_) => Ok(AppResponse::Unimplemented(request)),
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

/// Represents the available Conductor functions to call over an App interface
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum AppRequest {
    /// Get info about the App identified by the given `installed_app_id` argument,
    /// including info about each Cell installed by this App.
    /// Requires `installed_app_id` because an App interface can be the interface to multiple
    /// apps at the same time.
    ///
    /// Will be responded to with an [`AppResponse::AppInfo`]
    /// or an [`AppResponse::Error`]
    ///
    /// [`AppResponse::AppInfo`]: enum.AppResponse.html#variant.AppInfo
    /// [`AppResponse::Error`]: enum.AppResponse.html#variant.Error
    AppInfo {
        /// The InstalledAppId for which to get information
        installed_app_id: InstalledAppId,
    },
    /// Asks the conductor to do some crypto.
    ///
    /// Is currently unimplemented and will return
    /// an [`AppResponse::Unimplemented`](enum.AppResponse.html#variant.Unimplemented)
    Crypto(Box<CryptoRequest>),
    /// Call a zome function. See the inner [`ZomeCallInvocation`]
    /// struct to understand the data that must be provided.
    ///
    /// Will be responded to with an [`AppResponse::ZomeCallInvocation`]
    /// or an [`AppResponse::Error`]
    ///
    /// [`ZomeCallInvocation`]: ../../core/ribosome/struct.ZomeCallInvocation.html
    /// [`AppResponse::ZomeCallInvocation`]: enum.AppResponse.html#variant.ZomeCallInvocation
    /// [`AppResponse::Error`]: enum.AppResponse.html#variant.Error
    ZomeCallInvocation(Box<ZomeCallInvocation>),

    /// Update signal subscriptions.
    ///
    /// Is currently unimplemented and will return
    /// an [`AppResponse::Unimplemented`](enum.AppResponse.html#variant.Unimplemented)
    SignalSubscription(SignalSubscription),
}

/// Responses to requests received on an App interface
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum AppResponse {
    /// This request/response is unimplemented
    Unimplemented(AppRequest),

    /// Can occur in response to any [`AppRequest`].
    ///
    /// There has been an error during the handling of the request.
    /// See [`ExternalApiWireError`] for variants.
    ///
    /// [`AppRequest`]: enum.AppRequest.html
    /// [`ExternalApiWireError`]: error/enum.ExternalApiWireError.html
    Error(ExternalApiWireError),

    /// The succesful response to an [`AppRequest::AppInfo`].
    ///
    /// Option will be `None` if there is no installed app with the given `installed_app_id` value from the request.
    /// Check out [`InstalledApp`] for details on when the Option is `Some<InstalledApp>`
    ///
    /// [`InstalledApp`]: ../../../holochain_types/app/struct.InstalledApp.html
    /// [`AppRequest::AppInfo`]: enum.AppRequest.html#variant.AppInfo
    AppInfo(Option<InstalledApp>),

    /// The succesful response to an [`AppRequest::ZomeCallInvocation`].
    ///
    /// Note that [`ExternOutput`] is simply a structure of [`SerializedBytes`] so the client will have
    /// to decode this response back into the data provided by the Zome using a [msgpack](https://msgpack.org/) library to utilize it.
    ///
    /// [`AppRequest::ZomeCallInvocation`]: enum.AppRequest.html#variant.ZomeCallInvocation
    /// [`ExternOutput`]: ../../../holochain_zome_types/zome_io/struct.ExternOutput.html
    /// [`SerializedBytes`]: ../../../holochain_zome_types/query/struct.SerializedBytes.html
    ZomeCallInvocation(Box<ExternOutput>),
}

#[allow(missing_docs)]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum CryptoRequest {
    Sign(String),
    Decrypt(String),
    Encrypt(String),
}
