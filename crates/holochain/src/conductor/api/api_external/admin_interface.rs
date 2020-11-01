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
            AddAdminInterfaces(configs) => {
                self.conductor_handle
                    .clone()
                    .add_admin_interfaces(configs)
                    .await?;
                Ok(AdminResponse::AdminInterfacesAdded)
            }
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
                Ok(AdminResponse::DnasListed(dna_list))
            }
            GenerateAgentPubKey => {
                let agent_pub_key = self
                    .conductor_handle
                    .keystore()
                    .clone()
                    .generate_sign_keypair_from_pure_entropy()
                    .await?;
                Ok(AdminResponse::AgentPubKeyGenerated(agent_pub_key))
            }
            ListCellIds => {
                let cell_ids = self.conductor_handle.list_cell_ids().await?;
                Ok(AdminResponse::CellIdsListed(cell_ids))
            }
            ListActiveAppIds => {
                let app_ids = self.conductor_handle.list_active_app_ids().await?;
                Ok(AdminResponse::ActiveAppIdsListed(app_ids))
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
                Ok(AdminResponse::StateDumped(state))
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

/// Represents the available conductor functions to call over an Admin interface
/// and will result in a corresponding [`AdminResponse`] message being sent back over the
/// interface connection.
/// Enum variants follow a general convention of `verb_noun` as opposed to
/// the `noun_verb` of `AdminResponse`.
///
/// Expects a serialized object with any contents of the enum on a key `data`
/// and the enum variant on a key `type`, e.g.
/// `{ type: 'activate_app', data: { app_id: 'test_app' } }`
///
/// [`AdminResponse`]: enum.AdminResponse.html
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[cfg_attr(test, derive(Clone))]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum AdminRequest {
    /// Set up and register one or more new Admin interfaces
    /// as specified by a list of configurations. See [`AdminInterfaceConfig`]
    /// for details on the configuration.
    ///
    /// Will be responded to with an [`AdminResponse::AdminInterfacesAdded`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`AdminInterfaceConfig`]: ../config/struct.AdminInterfaceConfig.html
    /// [`AdminResponse::AdminInterfacesAdded`]: enum.AdminResponse.html#variant.AdminInterfacesAdded
    /// [`AdminResponse::Error`]: enum.AppResponse.html#variant.Error
    AddAdminInterfaces(Vec<AdminInterfaceConfig>),
    /// Install an app from a list of `Dna` paths.
    /// Triggers genesis to be run on all `Cell`s and to be stored.
    /// An `App` is intended for use by
    /// one and only one Agent and for that reason it takes an `AgentPubKey` and
    /// installs all the Dnas with that `AgentPubKey` forming new `Cell`s.
    /// See [`InstallAppPayload`] for full details on the configuration.
    ///
    /// Note that the new `App` will not be "activated" automatically after installation
    /// and can be activated by calling [`AdminRequest::ActivateApp`].
    ///
    /// Will be responded to with an [`AdminResponse::AppInstalled`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`InstallAppPayload`]: ../../../holochain_types/app/struct.InstallAppPayload.html
    /// [`AdminRequest::ActivateApp`]: enum.AdminRequest.html#variant.ActivateApp
    /// [`AdminResponse::AppInstalled`]: enum.AdminResponse.html#variant.AppInstalled
    /// [`AdminResponse::Error`]: enum.AppResponse.html#variant.Error
    InstallApp(Box<InstallAppPayload>),
    /// List the hashes of all installed `Dna`s.
    /// Takes no arguments.
    ///
    /// Will be responded to with an [`AdminResponse::DnasListed`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`AdminResponse::DnasListed`]: enum.AdminResponse.html#variant.DnasListed
    /// [`AdminResponse::Error`]: enum.AppResponse.html#variant.Error
    ListDnas,
    /// Generate a new AgentPubKey.
    /// Takes no arguments.
    ///
    /// Will be responded to with an [`AdminResponse::AgentPubKeyGenerated`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`AdminResponse::AgentPubKeyGenerated`]: enum.AdminResponse.html#variant.AgentPubKeyGenerated
    /// [`AdminResponse::Error`]: enum.AppResponse.html#variant.Error
    GenerateAgentPubKey,
    /// List all the cell ids in the conductor.
    /// Takes no arguments.
    ///
    /// Will be responded to with an [`AdminResponse::CellIdsListed`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`AdminResponse::CellIdsListed`]: enum.AdminResponse.html#variant.CellIdsListed
    /// [`AdminResponse::Error`]: enum.AppResponse.html#variant.Error
    ListCellIds,
    /// List the ids of all the active (activated) Apps in the conductor.
    /// Takes no arguments.
    ///
    /// Will be responded to with an [`AdminResponse::ActiveAppIdsListed`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`AdminResponse::ActiveAppIdsListed`]: enum.AdminResponse.html#variant.ActiveAppIdsListed
    /// [`AdminResponse::Error`]: enum.AppResponse.html#variant.Error
    ListActiveAppIds,
    /// Changes the `App` specified by argument `app_id` from an inactive state to an active state in the conductor,
    /// meaning that Zome calls can now be made and the `App` will be loaded on a reboot of the conductor.
    /// It is likely to want to call this after calling [`AdminRequest::InstallApp`], since a freshly
    /// installed `App` is not activated automatically.
    ///
    /// Will be responded to with an [`AdminResponse::AppActivated`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`AdminRequest::InstallApp`]: enum.AdminRequest.html#variant.InstallApp
    /// [`AdminResponse::AppActivated`]: enum.AdminResponse.html#variant.AppActivated
    /// [`AdminResponse::Error`]: enum.AppResponse.html#variant.Error
    ActivateApp {
        /// The AppId to activate
        app_id: AppId,
    },
    /// Changes the `App` specified by argument `app_id` from an active state to an inactive state in the conductor,
    /// meaning that Zome calls can no longer be made, and the `App` will not be loaded on a
    /// reboot of the conductor.
    ///
    /// Will be responded to with an [`AdminResponse::AppDeactivated`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`AdminResponse::AppDeactivated`]: enum.AdminResponse.html#variant.AppDeactivated
    /// [`AdminResponse::Error`]: enum.AppResponse.html#variant.Error
    DeactivateApp {
        /// The AppId to deactivate
        app_id: AppId,
    },
    /// Open up a new websocket interface at the networking port
    /// (optionally) specified by argument `port` (or using any free port if argument `port` is `None`)
    /// over which you can then use the [`AppRequest`] API.
    /// Any active `App` will be callable via this interface.
    /// The successful [`AdminResponse::AppInterfaceAttached`] message will contain
    /// the port chosen by the conductor if `None` was passed.
    ///
    /// Will be responded to with an [`AdminResponse::AppInterfaceAttached`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`AdminResponse::AppInterfaceAttached`]: enum.AdminResponse.html#variant.AppInterfaceAttached
    /// [`AdminResponse::Error`]: enum.AppResponse.html#variant.Error
    AttachAppInterface {
        /// Optional port, use None to let the
        /// OS choose a free port
        port: Option<u16>,
    },
    /// Dump the full state of the `Cell` specified by argument `cell_id`,
    /// including its chain, as a string containing JSON.
    ///
    /// Will be responded to with an [`AdminResponse::StateDumped`]
    /// or an [`AdminResponse::Error`]
    ///
    /// [`AdminResponse::Error`]: enum.AppResponse.html#variant.Error
    /// [`AdminResponse::StateDumped`]: enum.AdminResponse.html#variant.StateDumped
    DumpState {
        /// The `CellId` for which to dump state
        cell_id: Box<CellId>,
    },
}

/// Represents the possible responses to an [`AdminRequest`]
/// and follows a general convention of `noun_verb` as opposed to
/// the `verb_noun` of `AdminRequest`.
///
/// Will serialize as an object with any contents of the enum on a key `data`
/// and the enum variant on a key `type`, e.g.
/// `{ type: 'app_interface_attached', data: { port: 4000 } }`
///
/// [`AdminRequest`]: enum.AdminRequest.html
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[cfg_attr(test, derive(Clone))]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum AdminResponse {
    /// Can occur in response to any [`AdminRequest`].
    ///
    /// There has been an error during the handling of the request.
    /// See [`ExternalApiWireError`] for variants.
    ///
    /// [`AdminRequest`]: enum.AdminRequest.html
    /// [`ExternalApiWireError`]: error/enum.ExternalApiWireError.html
    Error(ExternalApiWireError),
    /// The succesful response to an [`AdminRequest::InstallApp`].
    ///
    /// The resulting [`InstalledApp`] contains the App id,
    /// the [`CellNick`]s and, most usefully, the new [`CellId`]s
    /// of the newly installed `Dna`s. See the [`InstalledApp`] docs for details.
    ///
    /// [`AdminRequest::InstallApp`]: enum.AdminRequest.html#variant.InstallApp
    /// [`InstalledApp`]: ../../../holochain_types/app/struct.InstalledApp.html
    /// [`CellNick`]: ../../../holochain_types/app/type.CellNick.html
    /// [`CellId`]: ../../../holochain_types/cell/struct.CellId.html
    AppInstalled(InstalledApp),
    /// The succesful response to an [`AdminRequest::AddAdminInterfaces`].
    ///
    /// It means the `AdminInterface`s have successfully been added
    ///
    /// [`AdminRequest::AddAdminInterfaces`]: enum.AdminRequest.html#variant.AddAdminInterfaces
    AdminInterfacesAdded,
    /// The succesful response to an [`AdminRequest::GenerateAgentPubKey`].
    ///
    /// Contains a new `AgentPubKey` generated by the Keystore
    ///
    /// [`AdminRequest::GenerateAgentPubKey`]: enum.AdminRequest.html#variant.GenerateAgentPubKey
    AgentPubKeyGenerated(AgentPubKey),
    /// The successful response to an [`AdminRequest::ListDnas`].
    ///
    /// Contains a list of the hashes of all installed `Dna`s
    ///
    /// [`AdminRequest::ListDnas`]: enum.AdminRequest.html#variant.ListDnas
    DnasListed(Vec<DnaHash>),
    /// The succesful response to an [`AdminRequest::ListCellIds`].
    ///
    /// Contains a list of all the `Cell` ids in the conductor
    ///
    /// [`AdminRequest::ListCellIds`]: enum.AdminRequest.html#variant.ListCellIds
    CellIdsListed(Vec<CellId>),
    /// The succesful response to an [`AdminRequest::ListActiveAppIds`].
    ///
    /// Contains a list of all the active `App` ids in the conductor
    ///
    /// [`AdminRequest::ListActiveAppIds`]: enum.AdminRequest.html#variant.ListActiveAppIds
    ActiveAppIdsListed(Vec<AppId>),
    /// The succesful response to an [`AdminRequest::AttachAppInterface`].
    ///
    /// `AppInterfaceApi` successfully attached.
    /// Contains the port number that was selected (if not specified) by Holochain
    /// for running this App interface
    ///
    /// [`AdminRequest::AttachAppInterface`]: enum.AdminRequest.html#variant.AttachAppInterface
    AppInterfaceAttached {
        /// Networking port of the new `AppInterfaceApi`
        port: u16,
    },
    /// The succesful response to an [`AdminRequest::ActivateApp`].
    ///
    /// It means the `App` was activated successfully
    ///
    /// [`AdminRequest::ActivateApp`]: enum.AdminRequest.html#variant.ActivateApp
    AppActivated,
    /// The succesful response to an [`AdminRequest::DeactivateApp`].
    ///
    /// It means the `App` was deactivated successfully.
    ///
    /// [`AdminRequest::DeactivateApp`]: enum.AdminRequest.html#variant.DeactivateApp
    AppDeactivated,
    /// The succesful response to an [`AdminRequest::DumpState`].
    ///
    /// The result contains a string of serialized JSON data which can be deserialized to access the
    /// full state dump, and inspect the source chain.
    ///
    /// [`AdminRequest::DumpState`]: enum.AdminRequest.html#variant.DumpState
    StateDumped(String),
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::conductor::Conductor;
    use anyhow::Result;
    use holochain_state::test_utils::{
        test_conductor_env, test_p2p_env, test_wasm_env, TestEnvironment,
    };
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
        let TestEnvironment {
            env: p2p_env,
            tmpdir: _p2p_tmpdir,
        } = test_p2p_env();
        let _tmpdir = test_env.tmpdir.clone();
        let handle = Conductor::builder()
            .test(test_env, wasm_env, p2p_env)
            .await?;
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
        assert_matches!(dna_list, AdminResponse::DnasListed(a) if a == expects);

        let res = admin_api
            .handle_admin_request(AdminRequest::ActivateApp {
                app_id: "test".to_string(),
            })
            .await;

        assert_matches!(res, AdminResponse::AppActivated);

        let res = admin_api
            .handle_admin_request(AdminRequest::ListCellIds)
            .await;

        assert_matches!(res, AdminResponse::CellIdsListed(v) if v == vec![cell_id]);

        let res = admin_api
            .handle_admin_request(AdminRequest::ListActiveAppIds)
            .await;

        assert_matches!(res, AdminResponse::ActiveAppIdsListed(v) if v == vec!["test".to_string()]);

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
