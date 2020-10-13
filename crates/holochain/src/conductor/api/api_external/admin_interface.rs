use super::InterfaceApi;
use crate::conductor::api::error::{
    ConductorApiError, ConductorApiResult, ExternalApiWireError, SerializationError,
};
use crate::conductor::{
    config::AdminInterfaceConfig,
    error::CreateAppError,
    interface::error::{InterfaceError, InterfaceResult},
    ConductorHandle,
};
use holo_hash::*;
use holochain_keystore::KeystoreSenderExt;
use holochain_serialized_bytes::prelude::*;
use holochain_types::{
    app::{AppId, InstallAppDnaPayload, InstallAppPayload, InstalledApp, InstalledCell},
    cell::CellId,
    dna::{DnaFile, JsonProperties},
};
use std::path::PathBuf;
use tracing::*;

/// A trait for the interface that a Conductor exposes to the outside world to use for administering the conductor.
/// This trait has a one mock implementation and one "Real" implementation
#[async_trait::async_trait]
pub trait AdminInterfaceApi: 'static + Send + Sync + Clone {
    /// Call an admin function to modify this Conductor's behavior
    async fn handle_admin_request_inner(
        &self,
        request: AdminRequest,
    ) -> ConductorApiResult<AdminResponse>;

    // -- provided -- //

    /// Deal with error cases produced by `handle_admin_request_inner`
    async fn handle_admin_request(&self, request: AdminRequest) -> AdminResponse {
        let res = self.handle_admin_request_inner(request).await;

        match res {
            Ok(response) => response,
            Err(e) => AdminResponse::Error(e.into()),
        }
    }
}

/// The admin interface that external connections
/// can use to make requests to the conductor
/// The concrete (non-mock) implementation of the AdminInterfaceApi
#[derive(Clone)]
pub struct RealAdminInterfaceApi {
    /// Mutable access to the Conductor
    conductor_handle: ConductorHandle,
}

impl RealAdminInterfaceApi {
    pub(crate) fn new(conductor_handle: ConductorHandle) -> Self {
        RealAdminInterfaceApi { conductor_handle }
    }
}

#[async_trait::async_trait]
impl AdminInterfaceApi for RealAdminInterfaceApi {
    async fn handle_admin_request_inner(
        &self,
        request: AdminRequest,
    ) -> ConductorApiResult<AdminResponse> {
        use AdminRequest::*;
        match request {
            AddAdminInterfaces(configs) => Ok(AdminResponse::AdminInterfacesAdded(
                self.conductor_handle
                    .clone()
                    .add_admin_interfaces(configs)
                    .await?,
            )),
            InstallApp(payload) => {
                trace!(?payload.dnas);
                let InstallAppPayload {
                    app_id,
                    agent_key,
                    dnas,
                } = *payload;

                // Install Dnas
                let tasks = dnas.into_iter().map(|dna_payload| async {
                    let InstallAppDnaPayload {
                        path,
                        properties,
                        membrane_proof,
                        nick,
                    } = dna_payload;
                    let dna = read_parse_dna(path, properties).await?;
                    let hash = dna.dna_hash().clone();
                    let cell_id = CellId::from((hash.clone(), agent_key.clone()));
                    self.conductor_handle.install_dna(dna).await?;
                    ConductorApiResult::Ok((InstalledCell::new(cell_id, nick), membrane_proof))
                });

                // Join all the install tasks
                let cell_ids_with_proofs = futures::future::join_all(tasks)
                    .await
                    .into_iter()
                    // Check all passed and return the proofs
                    .collect::<Result<Vec<_>, _>>()?;

                // Call genesis
                self.conductor_handle
                    .clone()
                    .install_app(app_id.clone(), cell_ids_with_proofs.clone())
                    .await?;

                let cell_data = cell_ids_with_proofs
                    .into_iter()
                    .map(|(cell_data, _)| cell_data)
                    .collect();
                let app = InstalledApp { app_id, cell_data };
                Ok(AdminResponse::AppInstalled(app))
            }
            ListDnas => {
                let dna_list = self.conductor_handle.list_dnas().await?;
                Ok(AdminResponse::ListDnas(dna_list))
            }
            GenerateAgentPubKey => {
                let agent_pub_key = self
                    .conductor_handle
                    .keystore()
                    .clone()
                    .generate_sign_keypair_from_pure_entropy()
                    .await?;
                Ok(AdminResponse::GenerateAgentPubKey(agent_pub_key))
            }
            ListCellIds => {
                let cell_ids = self.conductor_handle.list_cell_ids().await?;
                Ok(AdminResponse::ListCellIds(cell_ids))
            }
            ListActiveAppIds => {
                let app_ids = self.conductor_handle.list_active_app_ids().await?;
                Ok(AdminResponse::ListActiveAppIds(app_ids))
            }
            ActivateApp { app_id } => {
                // Activate app
                self.conductor_handle.activate_app(app_id.clone()).await?;

                // Create cells
                let errors = self.conductor_handle.clone().setup_cells().await?;

                // Check if this app was created successfully
                errors
                    .into_iter()
                    // We only care about this app for the activate command
                    .find(|cell_error| match cell_error {
                        CreateAppError::Failed {
                            app_id: error_app_id,
                            ..
                        } => error_app_id == &app_id,
                    })
                    // There was an error in this app so return it
                    .map(|this_app_error| Ok(AdminResponse::Error(this_app_error.into())))
                    // No error, return success
                    .unwrap_or(Ok(AdminResponse::AppActivated))
            }
            DeactivateApp { app_id } => {
                // Activate app
                self.conductor_handle.deactivate_app(app_id.clone()).await?;
                Ok(AdminResponse::AppDeactivated)
            }
            AttachAppInterface { port } => {
                let port = port.unwrap_or(0);
                let port = self
                    .conductor_handle
                    .clone()
                    .add_app_interface(port)
                    .await?;
                Ok(AdminResponse::AppInterfaceAttached { port })
            }
            DumpState { cell_id } => {
                let state = self.conductor_handle.dump_cell_state(&cell_id).await?;
                Ok(AdminResponse::JsonState(state))
            }
        }
    }
}

/// Reads the [Dna] from disk and parses to [SerializedBytes]
async fn read_parse_dna(
    dna_path: PathBuf,
    properties: Option<JsonProperties>,
) -> ConductorApiResult<DnaFile> {
    let dna_content = tokio::fs::read(dna_path)
        .await
        .map_err(|e| ConductorApiError::DnaReadError(format!("{:?}", e)))?;
    let mut dna = DnaFile::from_file_content(&dna_content).await?;
    if let Some(properties) = properties {
        let properties = SerializedBytes::try_from(properties).map_err(SerializationError::from)?;
        dna = dna.with_properties(properties).await?;
    }
    Ok(dna)
}

#[async_trait::async_trait]
impl InterfaceApi for RealAdminInterfaceApi {
    type ApiRequest = AdminRequest;
    type ApiResponse = AdminResponse;

    async fn handle_request(
        &self,
        request: Result<Self::ApiRequest, SerializedBytesError>,
    ) -> InterfaceResult<Self::ApiResponse> {
        // Don't hold the read across both awaits
        {
            self.conductor_handle
                .check_running()
                .await
                .map_err(Box::new)
                .map_err(InterfaceError::RequestHandler)?;
        }
        match request {
            Ok(request) => Ok(AdminInterfaceApi::handle_admin_request(self, request).await),
            Err(e) => Ok(AdminResponse::Error(SerializationError::from(e).into())),
        }
    }
}

/// The set of messages that a conductor understands how to handle over an Admin interface
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[cfg_attr(test, derive(Clone))]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum AdminRequest {
    /// Set up and register an Admin interface task
    AddAdminInterfaces(Vec<AdminInterfaceConfig>),
    /// Install an app from a list of Dna paths
    /// Triggers genesis to be run on all cells and
    /// Dnas to be stored
    InstallApp(Box<InstallAppPayload>),
    /// List all installed [Dna]s
    ListDnas,
    /// Generate a new AgentPubKey
    GenerateAgentPubKey,
    /// List all the cell ids in the conductor
    ListCellIds,
    /// List all the active app ids in the conductor
    ListActiveAppIds,
    /// Activate an app
    ActivateApp {
        /// The AppId to activate
        app_id: AppId,
    },
    /// Deactivate an app
    DeactivateApp {
        /// The AppId to deactivate
        app_id: AppId,
    },
    /// Attach a [AppInterfaceApi]
    AttachAppInterface {
        /// Optional port, use None to let the
        /// OS choose a free port
        port: Option<u16>,
    },
    /// Dump the state of a cell
    DumpState {
        /// The CellId for which to dump state
        cell_id: Box<CellId>,
    },
}

/// Responses to messages received on an Admin interface
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[cfg_attr(test, derive(Clone))]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum AdminResponse {
    /// This response is unimplemented
    Unimplemented(AdminRequest),
    /// hApp [Dna]s have successfully been installed
    AppInstalled(InstalledApp),
    /// AdminInterfaces have successfully been added
    AdminInterfacesAdded(()),
    /// A list of all installed [Dna]s
    ListDnas(Vec<DnaHash>),
    /// Keystore generated a new AgentPubKey
    GenerateAgentPubKey(AgentPubKey),
    /// Listing all the cell ids in the conductor
    ListCellIds(Vec<CellId>),
    /// Listing all the active app ids in the conductor
    ListActiveAppIds(Vec<AppId>),
    /// [AppInterfaceApi] successfully attached
    AppInterfaceAttached {
        /// Port of the new [AppInterfaceApi]
        port: u16,
    },
    /// An error has ocurred in this request
    Error(ExternalApiWireError),
    /// App activated successfully
    AppActivated,
    /// App deactivated successfully
    AppDeactivated,
    /// State of a cell
    JsonState(String),
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::conductor::Conductor;
    use anyhow::Result;
    use holochain_state::test_utils::{test_conductor_env, test_wasm_env, TestEnvironment};
    use holochain_types::{
        app::InstallAppDnaPayload,
        observability,
        test_utils::{fake_agent_pubkey_1, fake_dna_file, fake_dna_zomes, write_fake_dna_file},
    };
    use holochain_wasm_test_utils::TestWasm;
    use matches::assert_matches;
    use uuid::Uuid;

    #[tokio::test(threaded_scheduler)]
    async fn install_list_dna_app() -> Result<()> {
        observability::test_run().ok();
        let test_env = test_conductor_env();
        let TestEnvironment {
            env: wasm_env,
            tmpdir: _tmpdir,
        } = test_wasm_env();
        let _tmpdir = test_env.tmpdir.clone();
        let handle = Conductor::builder().test(test_env, wasm_env).await?;
        let shutdown = handle.take_shutdown_handle().await.unwrap();
        let admin_api = RealAdminInterfaceApi::new(handle.clone());
        let uuid = Uuid::new_v4();
        let dna = fake_dna_zomes(
            &uuid.to_string(),
            vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
        );
        let (dna_path, _tempdir) = write_fake_dna_file(dna.clone()).await.unwrap();
        let dna_payload = InstallAppDnaPayload::path_only(dna_path, "".to_string());
        let dna_hash = dna.dna_hash().clone();
        let agent_key = fake_agent_pubkey_1();
        let cell_id = CellId::new(dna.dna_hash().clone(), agent_key.clone());
        let expected_cell_ids = InstalledApp {
            app_id: "test".to_string(),
            cell_data: vec![InstalledCell::new(cell_id.clone(), "".to_string())],
        };
        let payload = InstallAppPayload {
            dnas: vec![dna_payload],
            app_id: "test".to_string(),
            agent_key,
        };

        let install_response = admin_api
            .handle_admin_request(AdminRequest::InstallApp(Box::new(payload)))
            .await;
        assert_matches!(
            install_response,
            AdminResponse::AppInstalled(cell_ids) if cell_ids == expected_cell_ids
        );
        let dna_list = admin_api.handle_admin_request(AdminRequest::ListDnas).await;
        let expects = vec![dna_hash];
        assert_matches!(dna_list, AdminResponse::ListDnas(a) if a == expects);

        let res = admin_api
            .handle_admin_request(AdminRequest::ActivateApp {
                app_id: "test".to_string(),
            })
            .await;

        assert_matches!(res, AdminResponse::AppActivated);

        let res = admin_api
            .handle_admin_request(AdminRequest::ListCellIds)
            .await;

        assert_matches!(res, AdminResponse::ListCellIds(v) if v == vec![cell_id]);

        let res = admin_api
            .handle_admin_request(AdminRequest::ListActiveAppIds)
            .await;

        assert_matches!(res, AdminResponse::ListActiveAppIds(v) if v == vec!["test".to_string()]);

        handle.shutdown().await;
        tokio::time::timeout(std::time::Duration::from_secs(1), shutdown)
            .await
            .ok();
        Ok(())
    }

    #[tokio::test(threaded_scheduler)]
    async fn dna_read_parses() -> Result<()> {
        let uuid = Uuid::new_v4();
        let dna = fake_dna_file(&uuid.to_string());
        let (dna_path, _tmpdir) = write_fake_dna_file(dna.clone()).await?;
        let json = serde_json::json!({
            "test": "example",
            "how_many": 42,
        });
        let properties = Some(JsonProperties::new(json.clone()));
        let result = read_parse_dna(dna_path, properties).await?;
        let properties = JsonProperties::new(json);
        let mut dna = dna.dna().clone();
        dna.properties = properties.try_into().unwrap();
        assert_eq!(&dna, result.dna());
        Ok(())
    }
}
