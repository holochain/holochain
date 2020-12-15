use super::{InterfaceApi, SignalSubscription};
use crate::conductor::{
    api::error::{ConductorApiResult, ExternalApiWireError, SerializationError},
    interface::error::{InterfaceError, InterfaceResult},
    state::AppInterfaceId,
    ConductorHandle,
};
use holo_hash::AgentPubKey;
use holochain_serialized_bytes::prelude::*;
use holochain_types::app::{InstalledApp, InstalledAppId};
use holochain_zome_types::{
    capability::CapSecret, cell::CellId, zome::FunctionName, zome::ZomeName, ExternInput,
    ExternOutput, ZomeCallResponse,
};

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
            AppRequest::ZomeCallInvocation(call) => {
                tracing::warn!(
                    "AppRequest::ZomeCallInvocation is deprecated, use AppRequest::ZomeCall (TODO: update conductor-api)"
                );
                self.handle_app_request_inner(AppRequest::ZomeCall(call))
                    .await
                    .map(|r| {
                        match r {
                            // if successful, re-wrap in the deprecated response type
                            AppResponse::ZomeCall(zc) => AppResponse::ZomeCallInvocation(zc),
                            // else (probably an error), return as-is
                            other => other,
                        }
                    })
            }
            AppRequest::ZomeCall(call) => {
                match self.conductor_handle.call_zome(*call.clone()).await? {
                    Ok(ZomeCallResponse::Ok(output)) => Ok(AppResponse::ZomeCall(Box::new(output))),
                    Ok(ZomeCallResponse::Unauthorized(_, _, _, _)) => Ok(AppResponse::Error(
                        ExternalApiWireError::ZomeCallUnauthorized(format!(
                            "No capabilities grant has been committed that allows the CapSecret {:?} to call the function {} in zome {}",
                            call.cap, call.fn_name, call.zome_name
                        )),
                    )),
                    Ok(ZomeCallResponse::NetworkError(e)) => unreachable!(
                        "Interface zome calls should never be routed to the network. This is a bug. Got {}",
                        e
                    ),
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
    /// Call a zome function. See the inner [`ZomeCall`]
    /// struct to understand the data that must be provided.
    ///
    /// Will be responded to with an [`AppResponse::ZomeCall`]
    /// or an [`AppResponse::Error`]
    ///
    /// [`ZomeCall`]: ../../core/ribosome/struct.ZomeCall.html
    /// [`AppResponse::ZomeCall`]: enum.AppResponse.html#variant.ZomeCall
    /// [`AppResponse::Error`]: enum.AppResponse.html#variant.Error
    ZomeCall(Box<ZomeCall>),

    /// DEPRECATED. Use `ZomeCall`.
    ZomeCallInvocation(Box<ZomeCall>),

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

    /// The successful response to an [`AppRequest::ZomeCall`].
    ///
    /// Note that [`ExternOutput`] is simply a structure of [`SerializedBytes`] so the client will have
    /// to decode this response back into the data provided by the Zome using a [msgpack](https://msgpack.org/) library to utilize it.
    ///
    /// [`AppRequest::ZomeCall`]: enum.AppRequest.html#variant.ZomeCall
    /// [`ExternOutput`]: ../../../holochain_zome_types/zome_io/struct.ExternOutput.html
    /// [`SerializedBytes`]: ../../../holochain_zome_types/query/struct.SerializedBytes.html
    ZomeCall(Box<ExternOutput>),

    /// DEPRECATED. See `ZomeCall`.
    ZomeCallInvocation(Box<ExternOutput>),
}

/// The data provided across an App interface in order to make a zome call
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ZomeCall {
    /// The Id of the `Cell` containing the Zome to be called
    pub cell_id: CellId,
    /// The Zome containing the function to be called
    pub zome_name: ZomeName,
    /// The name of the Zome function to call
    pub fn_name: FunctionName,
    /// The serialized data to pass as an argument to the Zome call
    pub payload: ExternInput,
    /// The capability request authorization.
    /// This can be `None` and still succeed in the case where the function
    /// in the zome being called has been given an Unrestricted status
    /// via a `CapGrant`. Otherwise, it will be necessary to provide a `CapSecret` for every call.
    pub cap: Option<CapSecret>,
    /// The provenance (source) of the call.
    ///
    /// NB: **This will go away** as soon as Holochain has a way of determining who
    /// is making this ZomeCall over this interface. Until we do, the caller simply
    /// provides this data and Holochain trusts them.
    pub provenance: AgentPubKey,
}

#[allow(missing_docs)]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum CryptoRequest {
    Sign(String),
    Decrypt(String),
    Encrypt(String),
}
