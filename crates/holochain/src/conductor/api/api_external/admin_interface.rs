use super::InterfaceApi;
use crate::conductor::api::error::ConductorApiError;
use crate::conductor::api::error::ConductorApiResult;

use crate::conductor::api::error::SerializationError;

use crate::conductor::error::CreateAppError;
use crate::conductor::interface::error::InterfaceError;
use crate::conductor::interface::error::InterfaceResult;
use crate::conductor::ConductorHandle;
use holochain_keystore::KeystoreSenderExt;
use holochain_serialized_bytes::prelude::*;
use holochain_types::prelude::*;

use holochain_zome_types::cell::CellId;

use std::path::PathBuf;
use tracing::*;

pub use holochain_conductor_api::*;

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
            RegisterDna(payload) => {
                trace!(register_dna_payload = ?payload);
                let mut dna = match payload.source {
                    DnaSource::Hash(ref hash) => {
                        if payload.properties.is_none() && payload.uuid.is_none() {
                            return Err(ConductorApiError::DnaReadError(
                                "Hash Dna source requires properties or uuid to create a derived Dna"
                                    .to_string(),
                            ));
                        }
                        self.conductor_handle.get_dna(hash).await.ok_or_else(|| {
                            ConductorApiError::DnaReadError(format!(
                                "Unable to create derived Dna: {} not registered",
                                hash
                            ))
                        })?
                    }
                    DnaSource::Path(path) => read_parse_dna(path, None).await?, // properties handled below
                    DnaSource::DnaFile(dna) => dna,
                };
                if let Some(props) = payload.properties {
                    let properties =
                        SerializedBytes::try_from(props).map_err(SerializationError::from)?;
                    dna = dna.with_properties(properties).await?;
                }
                if let Some(uuid) = payload.uuid {
                    dna = dna.with_uuid(uuid).await?;
                }
                let hash = dna.dna_hash().clone();
                let dna_list = self.conductor_handle.list_dnas().await?;
                if dna_list.contains(&hash) {
                    info!("there");
                    return Err(ConductorApiError::DnaReadError(
                        "Given dna has already been registered".to_string(),
                    ));
                }
                self.conductor_handle.install_dna(dna).await?;
                Ok(AdminResponse::DnaRegistered(hash))
            }
            InstallApp(payload) => {
                trace!(?payload.dnas);
                let InstallAppPayload {
                    installed_app_id,
                    agent_key,
                    dnas,
                } = *payload;

                // Install Dnas
                let tasks = dnas.into_iter().map(|dna_payload| async {
                    let InstallAppDnaPayload {
                        path: maybe_path,
                        hash: maybe_hash,
                        properties,
                        membrane_proof,
                        nick,
                    } = dna_payload;
                    if maybe_path.is_none() && maybe_hash.is_none() {
                        return Err(ConductorApiError::DnaReadError("Neither path nor hash specified in payload".to_string()))
                    };
                    if maybe_path.is_some() && maybe_hash.is_some() {
                        return Err(ConductorApiError::DnaReadError("Both path and hash specified in payload, pick just one".to_string()))
                    }
                    if let Some(path) = maybe_path {
                        // TODO: this if let will be removed after deprecation period
                        tracing::warn!("specifying dna by path with register side-effect is deprecated, please use RegisterDna and install by hash");
                        let dna = read_parse_dna(path, properties).await?;
                        let hash = dna.dna_hash().clone();
                        let cell_id = CellId::from((hash.clone(), agent_key.clone()));
                        self.conductor_handle.install_dna(dna).await?;
                        ConductorApiResult::Ok((InstalledCell::new(cell_id, nick), membrane_proof))
                    } else if let Some(hash) = maybe_hash {
                        // confirm that hash has been installed
                        let dna_list = self.conductor_handle.list_dnas().await?;
                        if !dna_list.contains(&hash) {
                            return Err(ConductorApiError::DnaReadError(format!("Given dna has not been registered: {}", hash)));
                        }
                        let cell_id = CellId::from((hash.clone(), agent_key.clone()));
                        ConductorApiResult::Ok((InstalledCell::new(cell_id, nick), membrane_proof))
                    } else {
                        unreachable!()
                    }
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
                    .install_app(installed_app_id.clone(), cell_ids_with_proofs.clone())
                    .await?;

                let cell_data = cell_ids_with_proofs
                    .into_iter()
                    .map(|(cell_data, _)| cell_data)
                    .collect();
                let app = InstalledApp {
                    installed_app_id,
                    cell_data,
                };
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
            ListActiveApps => {
                let app_ids = self.conductor_handle.list_active_apps().await?;
                Ok(AdminResponse::ActiveAppsListed(app_ids))
            }
            ActivateApp { installed_app_id } => {
                // Activate app
                self.conductor_handle
                    .activate_app(installed_app_id.clone())
                    .await?;

                // Create cells
                let errors = self.conductor_handle.clone().setup_cells().await?;

                // Check if this app was created successfully
                errors
                    .into_iter()
                    // We only care about this app for the activate command
                    .find(|cell_error| match cell_error {
                        CreateAppError::Failed {
                            installed_app_id: error_app_id,
                            ..
                        } => error_app_id == &installed_app_id,
                    })
                    // There was an error in this app so return it
                    .map(|this_app_error| Ok(AdminResponse::Error(this_app_error.into())))
                    // No error, return success
                    .unwrap_or(Ok(AdminResponse::AppActivated))
            }
            DeactivateApp { installed_app_id } => {
                // Activate app
                self.conductor_handle
                    .deactivate_app(installed_app_id.clone())
                    .await?;
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
            AddAgentInfo { agent_infos } => {
                self.conductor_handle.add_agent_infos(agent_infos).await?;
                Ok(AdminResponse::AgentInfoAdded)
            }
            RequestAgentInfo { cell_id } => {
                let r = self.conductor_handle.get_agent_infos(cell_id).await?;
                Ok(AdminResponse::AgentInfoRequested(r))
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::conductor::Conductor;
    use anyhow::Result;
    use holochain_lmdb::test_utils::test_environments;
    use holochain_types::app::InstallAppDnaPayload;
    use holochain_types::test_utils::fake_agent_pubkey_1;
    use holochain_types::test_utils::fake_dna_file;
    use holochain_types::test_utils::fake_dna_zomes;
    use holochain_types::test_utils::write_fake_dna_file;
    use holochain_wasm_test_utils::TestWasm;
    use matches::assert_matches;
    use observability;
    use uuid::Uuid;

    #[tokio::test(threaded_scheduler)]
    async fn register_list_dna_app() -> Result<()> {
        observability::test_run().ok();
        let envs = test_environments();
        let handle = Conductor::builder().test(&envs).await?;
        let shutdown = handle.take_shutdown_handle().await.unwrap();
        let admin_api = RealAdminInterfaceApi::new(handle.clone());
        let uuid = Uuid::new_v4();
        let dna = fake_dna_zomes(
            &uuid.to_string(),
            vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
        );
        let dna_hash = dna.dna_hash().clone();
        let (dna_path, _tempdir) = write_fake_dna_file(dna.clone()).await.unwrap();
        let mut path_payload = RegisterDnaPayload {
            uuid: None,
            properties: None,
            source: DnaSource::Path(dna_path),
        };
        let path0_install_response = admin_api
            .handle_admin_request(AdminRequest::RegisterDna(Box::new(path_payload.clone())))
            .await;
        assert_matches!(
            path0_install_response,
            AdminResponse::DnaRegistered(h) if h == dna_hash
        );

        // re-register
        let path1_install_response = admin_api
            .handle_admin_request(AdminRequest::RegisterDna(Box::new(path_payload.clone())))
            .await;
        assert_matches!(
            path1_install_response,
            AdminResponse::Error(ExternalApiWireError::DnaReadError(e)) if e == String::from("Given dna has already been registered")
        );

        let dna_list = admin_api.handle_admin_request(AdminRequest::ListDnas).await;
        let expects = vec![dna_hash.clone()];
        assert_matches!(dna_list, AdminResponse::DnasListed(a) if a == expects);

        // register by hash
        let mut hash_payload = RegisterDnaPayload {
            uuid: None,
            properties: None,
            source: DnaSource::Hash(dna_hash.clone()),
        };

        // without properties or uuid should throw error
        let hash_install_response = admin_api
            .handle_admin_request(AdminRequest::RegisterDna(Box::new(hash_payload.clone())))
            .await;
        assert_matches!(
            hash_install_response,
            AdminResponse::Error(ExternalApiWireError::DnaReadError(e)) if e == String::from("Hash Dna source requires properties or uuid to create a derived Dna")
        );

        // with a property should install and produce a different hash
        let json = serde_json::json!({
            "some prop": "foo",
        });
        hash_payload.properties = Some(JsonProperties::new(json.clone()));
        let install_response = admin_api
            .handle_admin_request(AdminRequest::RegisterDna(Box::new(hash_payload.clone())))
            .await;
        assert_matches!(
            install_response,
            AdminResponse::DnaRegistered(hash) if hash != dna_hash
        );

        // with a uuid should install and produce a different hash
        hash_payload.properties = None;
        hash_payload.uuid = Some(String::from("12345678900000000000000"));
        let hash2_install_response = admin_api
            .handle_admin_request(AdminRequest::RegisterDna(Box::new(hash_payload)))
            .await;
        assert_matches!(
            hash2_install_response,
            AdminResponse::DnaRegistered(hash) if hash != dna_hash
        );

        // from a path with a same uuid should be already registered
        path_payload.uuid = Some(String::from("12345678900000000000000"));
        let path2_install_response = admin_api
            .handle_admin_request(AdminRequest::RegisterDna(Box::new(path_payload.clone())))
            .await;
        assert_matches!(
            path2_install_response,
            AdminResponse::Error(ExternalApiWireError::DnaReadError(e)) if e == String::from("Given dna has already been registered")
        );

        // from a path with different uuid should produce different hash
        path_payload.uuid = Some(String::from("foo"));
        let path3_install_response = admin_api
            .handle_admin_request(AdminRequest::RegisterDna(Box::new(path_payload)))
            .await;
        assert_matches!(
            path3_install_response,
            AdminResponse::DnaRegistered(hash) if hash != dna_hash
        );

        handle.shutdown().await;
        tokio::time::timeout(std::time::Duration::from_secs(1), shutdown)
            .await
            .ok();
        Ok(())
    }

    #[tokio::test(threaded_scheduler)]
    async fn install_list_dna_app() -> Result<()> {
        observability::test_run().ok();
        let envs = test_environments();
        let handle = Conductor::builder().test(&envs).await?;
        let shutdown = handle.take_shutdown_handle().await.unwrap();
        let admin_api = RealAdminInterfaceApi::new(handle.clone());
        let uuid = Uuid::new_v4();
        let dna = fake_dna_zomes(
            &uuid.to_string(),
            vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
        );
        let (dna_path, _tempdir) = write_fake_dna_file(dna.clone()).await.unwrap();
        let agent_key1 = fake_agent_pubkey_1();

        // attempt install with a hash before the DNA has been registered
        let dna_hash = dna.dna_hash();
        let hash_payload = InstallAppDnaPayload::hash_only(dna_hash.clone(), "".to_string());
        let hash_install_payload = InstallAppPayload {
            dnas: vec![hash_payload],
            installed_app_id: "test-by-hash".to_string(),
            agent_key: agent_key1,
        };
        let install_response = admin_api
            .handle_admin_request(AdminRequest::InstallApp(Box::new(
                hash_install_payload.clone(),
            )))
            .await;
        assert_matches!(
            install_response,
            AdminResponse::Error(ExternalApiWireError::DnaReadError(e)) if e == format!("Given dna has not been registered: {}", dna_hash)
        );

        // now install it using the path which should add the dna to the database
        let agent_key2 = fake_agent_pubkey_2();
        let path_payload = InstallAppDnaPayload::path_only(dna_path, "".to_string());
        let cell_id2 = CellId::new(dna_hash.clone(), agent_key2.clone());
        let expected_cell_ids = InstalledApp {
            installed_app_id: "test-by-path".to_string(),
            cell_data: vec![InstalledCell::new(cell_id2.clone(), "".to_string())],
        };
        let path_install_payload = InstallAppPayload {
            dnas: vec![path_payload],
            installed_app_id: "test-by-path".to_string(),
            agent_key: agent_key2,
        };

        let install_response = admin_api
            .handle_admin_request(AdminRequest::InstallApp(Box::new(path_install_payload)))
            .await;
        assert_matches!(
            install_response,
            AdminResponse::AppInstalled(cell_ids) if cell_ids == expected_cell_ids
        );
        let dna_list = admin_api.handle_admin_request(AdminRequest::ListDnas).await;
        let expects = vec![dna_hash.clone()];
        assert_matches!(dna_list, AdminResponse::DnasListed(a) if a == expects);

        let res = admin_api
            .handle_admin_request(AdminRequest::ActivateApp {
                installed_app_id: "test-by-path".to_string(),
            })
            .await;
        assert_matches!(res, AdminResponse::AppActivated);

        let res = admin_api
            .handle_admin_request(AdminRequest::ListCellIds)
            .await;

        assert_matches!(res, AdminResponse::CellIdsListed(v) if v == vec![cell_id2]);

        // now try to install the happ using the hash
        let _install_response = admin_api
            .handle_admin_request(AdminRequest::InstallApp(Box::new(hash_install_payload)))
            .await;
        let _res = admin_api
            .handle_admin_request(AdminRequest::ActivateApp {
                installed_app_id: "test-by-hash".to_string(),
            })
            .await;

        let res = admin_api
            .handle_admin_request(AdminRequest::ListActiveApps)
            .await;

        assert_matches!(res, AdminResponse::ActiveAppsListed(v) if v.contains(&"test-by-path".to_string()) && v.contains(&"test-by-hash".to_string())
        );

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
        let mut dna = dna.dna_def().clone();
        dna.properties = properties.try_into().unwrap();
        assert_eq!(&dna, result.dna_def());
        Ok(())
    }
}
