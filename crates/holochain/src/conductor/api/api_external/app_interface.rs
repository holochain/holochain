use crate::conductor::api::error::ConductorApiError;
use crate::conductor::api::error::ConductorApiResult;
use crate::conductor::api::error::SerializationError;
use crate::conductor::interface::error::InterfaceError;
use crate::conductor::interface::error::InterfaceResult;
use crate::conductor::ConductorHandle;

use holochain_serialized_bytes::prelude::*;

use holochain_types::prelude::*;

pub use holochain_conductor_api::*;

/// The Conductor lives inside an Arc<RwLock<_>> which is shared with all
/// other Api references
#[derive(Clone)]
pub struct AppInterfaceApi {
    conductor_handle: ConductorHandle,
}

impl AppInterfaceApi {
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
            Ok(request) => Ok(self.handle_app_request(installed_app_id, request).await),
            Err(e) => Ok(AppResponse::Error(SerializationError::from(e).into())),
        }
    }

    /// Deal with error cases produced by `handle_app_request_inner`
    async fn handle_app_request(
        &self,
        installed_app_id: InstalledAppId,
        request: AppRequest,
    ) -> AppResponse {
        tracing::debug!("app request: {:?}", request);

        let res = self
            .handle_app_request_inner(installed_app_id, request)
            .await
            .unwrap_or_else(|e| AppResponse::Error(e.into()));
        tracing::debug!("app response: {:?}", res);
        res
    }

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
            AppRequest::CallZome(zome_call_params_signed) => {
                let zome_call_params = zome_call_params_signed
                    .bytes
                    .clone()
                    .decode::<ZomeCallParams>()
                    .map_err(|e| ConductorApiError::SerializationError(e.into()))?;
                if !is_valid_signature(
                    &zome_call_params.provenance,
                    zome_call_params_signed.bytes.as_bytes(),
                    &zome_call_params_signed.signature,
                )
                .await?
                {
                    return Ok(AppResponse::Error(
                        ExternalApiWireError::ZomeCallAuthenticationFailed(format!(
                            "Authentication failure. Bad signature {:?} by provenance {:?}.",
                            zome_call_params_signed.signature, zome_call_params.provenance,
                        )),
                    ));
                }

                match self.conductor_handle.call_zome(zome_call_params.clone()).await? {
                    Ok(ZomeCallResponse::Ok(output)) => Ok(AppResponse::ZomeCalled(Box::new(output))),
                    Ok(ZomeCallResponse::Unauthorized(zome_call_authorization, _, zome_name, fn_name, _)) => Ok(AppResponse::Error(
                        ExternalApiWireError::ZomeCallUnauthorized(format!(
                            "Call was not authorized with reason {:?}, cap secret {:?} to call the function {} in zome {}",
                            zome_call_authorization, zome_call_params.cap_secret, fn_name, zome_name
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
            #[cfg(feature = "unstable-countersigning")]
            AppRequest::GetCountersigningSessionState(payload) => {
                let countersigning_session_state = self
                    .conductor_handle
                    .clone()
                    .get_countersigning_session_state(&payload)
                    .await?;
                Ok(AppResponse::CountersigningSessionState(Box::new(
                    countersigning_session_state,
                )))
            }
            #[cfg(feature = "unstable-countersigning")]
            AppRequest::AbandonCountersigningSession(payload) => {
                self.conductor_handle
                    .clone()
                    .abandon_countersigning_session(&payload)
                    .await?;
                Ok(AppResponse::CountersigningSessionAbandoned)
            }
            #[cfg(feature = "unstable-countersigning")]
            AppRequest::PublishCountersigningSession(payload) => {
                self.conductor_handle
                    .clone()
                    .publish_countersigning_session(&payload)
                    .await?;
                Ok(AppResponse::PublishCountersigningSessionTriggered)
            }
            AppRequest::CreateCloneCell(payload) => {
                let clone_cell = self
                    .conductor_handle
                    .clone()
                    .create_clone_cell(&installed_app_id, *payload)
                    .await?;
                Ok(AppResponse::CloneCellCreated(clone_cell))
            }
            AppRequest::DisableCloneCell(payload) => {
                self.conductor_handle
                    .clone()
                    .disable_clone_cell(&installed_app_id, &payload)
                    .await?;
                Ok(AppResponse::CloneCellDisabled)
            }
            AppRequest::EnableCloneCell(payload) => {
                let enabled_cell = self
                    .conductor_handle
                    .clone()
                    .enable_clone_cell(&installed_app_id, &payload)
                    .await?;
                Ok(AppResponse::CloneCellEnabled(enabled_cell))
            }
            AppRequest::NetworkInfo(payload) => {
                let info = self
                    .conductor_handle
                    .network_info(&installed_app_id, &payload)
                    .await?;
                Ok(AppResponse::NetworkInfo(info))
            }
            AppRequest::ListWasmHostFunctions => Ok(AppResponse::ListWasmHostFunctions(
                self.conductor_handle.list_wasm_host_functions().await?,
            )),
            AppRequest::ProvideMemproofs(memproofs) => {
                self.conductor_handle
                    .clone()
                    .provide_memproofs(&installed_app_id, memproofs)
                    .await?;
                Ok(AppResponse::Ok)
            }
            AppRequest::EnableApp => {
                let status = self
                    .conductor_handle
                    .get_app_info(&installed_app_id)
                    .await?
                    .ok_or(ConductorApiError::other("app not found".to_string()))?
                    .status;
                match status {
                    AppInfoStatus::Running
                    | AppInfoStatus::Disabled {
                        reason: DisabledAppReason::NotStartedAfterProvidingMemproofs,
                    } => {
                        self.conductor_handle
                            .clone()
                            .enable_app(installed_app_id.clone())
                            .await?;
                        Ok(AppResponse::Ok)
                    }
                    _ => Err(ConductorApiError::other(
                        "app not in correct state to enable".to_string(),
                    )),
                }
            } //
              // TODO: implement after DPKI lands
              // AppRequest::RotateAppAgentKey => {
              //     let new_key = self
              //         .conductor_handle
              //         .rotate_app_agent_key(&installed_app_id)
              //         .await?;
              //     Ok(AppResponse::AppAgentKeyRotated(new_key))
              // }
        }
    }
}

/// The payload for authenticating an app interface connection
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct AppAuthentication {
    /// The token received from the admin interface, demonstrating that the app is allowed
    /// to connect.
    pub token: Vec<u8>,

    /// If the app interface is bound to an installed app, this is the ID of that app. This field
    /// must be provided by Holochain and not the client.
    pub installed_app_id: Option<InstalledAppId>,
}

pub(crate) async fn is_valid_signature(
    provenance: &AgentPubKey,
    bytes: &[u8],
    signature: &Signature,
) -> ConductorApiResult<bool> {
    // Signature is verified against the hash of the signed zome call parameter bytes.
    let bytes_hash = sha2_512(bytes);
    Ok(provenance
        .verify_signature_raw(signature, bytes_hash.into())
        .await?)
}

#[cfg(test)]
mod tests {
    use holo_hash::{sha2_512, AgentPubKey};
    use holochain_keystore::{test_keystore, AgentPubKeyExt};
    use holochain_types::prelude::Signature;

    use super::is_valid_signature;

    #[tokio::test(flavor = "multi_thread")]
    async fn valid_signature() {
        let keystore = test_keystore();
        let agent_key = keystore.new_sign_keypair_random().await.unwrap();
        let bytes = vec![0u8];
        let bytes_hash = sha2_512(&bytes);
        let signature = agent_key
            .sign_raw(&keystore, bytes_hash.into())
            .await
            .unwrap();
        let is_valid = is_valid_signature(&agent_key, &bytes, &signature)
            .await
            .unwrap();
        assert!(is_valid);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn invalid_signature() {
        let keystore = test_keystore();
        let agent_key = keystore.new_sign_keypair_random().await.unwrap();
        let bytes = vec![0u8];
        let signature = Signature::from([0u8; 64]);
        let is_valid = is_valid_signature(&agent_key, &bytes, &signature)
            .await
            .unwrap();
        assert!(!is_valid);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn invalid_provenance() {
        let agent_key = AgentPubKey::from_raw_32(vec![0u8; 32]);
        let bytes = vec![0u8];
        let signature = Signature::from([0u8; 64]);
        let is_valid = is_valid_signature(&agent_key, &bytes, &signature)
            .await
            .unwrap();
        assert!(!is_valid);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn valid_signature_but_different_provenance() {
        let keystore = test_keystore();
        let signer_key = keystore.new_sign_keypair_random().await.unwrap();
        let bytes = vec![0u8];
        let bytes_hash = sha2_512(&bytes);
        let signature = signer_key.sign_raw(&keystore, bytes.into()).await.unwrap();
        let provenance = keystore.new_sign_keypair_random().await.unwrap();
        let is_valid = is_valid_signature(&provenance, &bytes_hash, &signature)
            .await
            .unwrap();
        assert!(!is_valid);
    }
}
