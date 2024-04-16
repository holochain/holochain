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
        installed_app_id: InstalledAppId,
        request: AppRequest,
    ) -> ConductorApiResult<AppResponse>;

    // -- provided -- //

    /// Deal with error cases produced by `handle_app_request_inner`
    async fn handle_app_request(
        &self,
        installed_app_id: InstalledAppId,
        request: AppRequest,
    ) -> AppResponse {
        tracing::debug!("app request: {:?}", request);

        let res = match self
            .handle_app_request_inner(installed_app_id, request)
            .await
        {
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

    /// Check an authentication request and return the app that access has been granted
    /// for on success.
    pub async fn auth(&self, auth: AppAuthentication) -> InterfaceResult<InstalledAppId> {
        self.conductor_handle
            .authenticate_app_token(auth.token, auth.installed_app_id)
            .map_err(Box::new)
            .map_err(InterfaceError::RequestHandler)
    }

    /// Handle an [AppRequest] in the context of an [InstalledAppId], and return an [AppResponse].
    pub async fn handle_request(
        &self,
        installed_app_id: InstalledAppId,
        request: Result<AppRequest, SerializedBytesError>,
    ) -> InterfaceResult<AppResponse> {
        {
            self.conductor_handle
                .check_running()
                .map_err(Box::new)
                .map_err(InterfaceError::RequestHandler)?;
        }
        match request {
            Ok(request) => {
                Ok(AppInterfaceApi::handle_app_request(self, installed_app_id, request).await)
            }
            Err(e) => Ok(AppResponse::Error(SerializationError::from(e).into())),
        }
    }
}

#[async_trait::async_trait]
impl AppInterfaceApi for RealAppInterfaceApi {
    /// Routes the [AppRequest] to the [AppResponse]
    async fn handle_app_request_inner(
        &self,
        installed_app_id: InstalledAppId,
        request: AppRequest,
    ) -> ConductorApiResult<AppResponse> {
        match request {
            AppRequest::AppInfo => Ok(AppResponse::AppInfo(
                self.conductor_handle
                    .get_app_info(&installed_app_id)
                    .await?,
            )),
            AppRequest::CallZome(call) => {
                match self.conductor_handle.call_zome(*call.clone()).await? {
                    Ok(ZomeCallResponse::Ok(output)) => Ok(AppResponse::ZomeCalled(Box::new(output))),
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
                let clone_cell = self
                    .conductor_handle
                    .clone()
                    .create_clone_cell(*payload)
                    .await?;
                Ok(AppResponse::CloneCellCreated(clone_cell))
            }
            AppRequest::DisableCloneCell(payload) => {
                self.conductor_handle
                    .clone()
                    .disable_clone_cell(&payload)
                    .await?;
                Ok(AppResponse::CloneCellDisabled)
            }
            AppRequest::EnableCloneCell(payload) => {
                let enabled_cell = self
                    .conductor_handle
                    .clone()
                    .enable_clone_cell(&payload)
                    .await?;
                Ok(AppResponse::CloneCellEnabled(enabled_cell))
            }
            AppRequest::NetworkInfo(payload) => {
                let info = self.conductor_handle.network_info(&payload).await?;
                Ok(AppResponse::NetworkInfo(info))
            }
            AppRequest::ListWasmHostFunctions => Ok(AppResponse::ListWasmHostFunctions(
                self.conductor_handle.list_wasm_host_functions().await?,
            )),
        }
    }
}

/// TODO document me please
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct AppAuthentication {
    /// TODO ME TOO!
    pub token: Vec<u8>,

    /// TODO ME TOO!
    pub installed_app_id: Option<InstalledAppId>,
}
