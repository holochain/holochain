use super::InterfaceApi;
use crate::conductor::api::error::ConductorApiResult;
use crate::conductor::api::error::ExternalApiWireError;
use crate::conductor::api::error::SerializationError;
use crate::conductor::interface::error::InterfaceError;
use crate::conductor::interface::error::InterfaceResult;
use crate::conductor::ConductorHandle;

use holochain_serialized_bytes::prelude::*;

use holochain_types::prelude::*;

pub use holochain_conductor_api::*;

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
        tracing::debug!("app request: {:?}", request);

        let res = match self.handle_app_request_inner(request).await {
            Ok(response) => response,
            Err(e) => AppResponse::Error(e.into()),
        };
        tracing::debug!("app response: {:?}", res);
        res
    }
}

/// The Conductor lives inside an Arc<RwLock<_>> which is shared with all
/// other Api references
#[derive(Clone)]
pub struct RealAppInterfaceApi {
    conductor_handle: ConductorHandle,
}

impl RealAppInterfaceApi {
    /// Create a new instance from a shared Conductor reference
    pub fn new(conductor_handle: ConductorHandle) -> Self {
        Self { conductor_handle }
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
            #[allow(deprecated)]
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
                    Ok(ZomeCallResponse::Unauthorized(zome_call_authorization, _, zome_name, fn_name, _)) => Ok(AppResponse::Error(
                        ExternalApiWireError::ZomeCallUnauthorized(format!(
                            "Call was not authorized with reason {:?}, cap secret {:?} to call the function {} in zome {}",
                            zome_call_authorization, call.cap_secret, fn_name, zome_name
                        )),
                    )),
                    Ok(ZomeCallResponse::NetworkError(e)) => unreachable!(
                        "Interface zome calls should never be routed to the network. This is a bug. Got {}",
                        e
                    ),
                    Ok(ZomeCallResponse::CountersigningSession(e)) => Ok(AppResponse::Error(
                        ExternalApiWireError::CountersigningSessionError(format!(
                            "A countersigning session has failed to start on this zome call because: {}",
                            e
                        )),
                    )),
                    Err(e) => Ok(AppResponse::Error(e.into())),
                }
            }
            AppRequest::CreateCloneCell(payload) => {
                let installed_clone_cell = self
                    .conductor_handle
                    .clone()
                    .create_clone_cell(*payload)
                    .await?;
                Ok(AppResponse::CloneCellCreated(installed_clone_cell))
            }
            AppRequest::ArchiveCloneCell(payload) => {
                self.conductor_handle
                    .clone()
                    .archive_clone_cell(&*payload)
                    .await?;
                Ok(AppResponse::CloneCellArchived)
            }
            AppRequest::GossipInfo(payload) => {
                let info = self.conductor_handle.gossip_info(&payload.dnas).await?;
                Ok(AppResponse::GossipInfo(info))
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
                .map_err(Box::new)
                .map_err(InterfaceError::RequestHandler)?;
        }
        match request {
            Ok(request) => Ok(AppInterfaceApi::handle_app_request(self, request).await),
            Err(e) => Ok(AppResponse::Error(SerializationError::from(e).into())),
        }
    }
}
