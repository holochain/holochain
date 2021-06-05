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
use holochain_types::dna::DnaBundle;
use holochain_types::prelude::*;
use mr_bundle::Bundle;

use holochain_zome_types::cell::CellId;

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
                let RegisterDnaPayload {
                    uid,
                    properties,
                    source,
                } = *payload;
                // uid and properties from the register call will override any in the bundle
                let dna = match source {
                    DnaSource::Hash(ref hash) => {
                        if properties.is_none() && uid.is_none() {
                            return Err(ConductorApiError::DnaReadError(
                                "Hash Dna source requires properties or uid to create a derived Dna"
                                    .to_string(),
                            ));
                        }
                        let mut dna =
                            self.conductor_handle.get_dna(hash).await.ok_or_else(|| {
                                ConductorApiError::DnaReadError(format!(
                                    "Unable to create derived Dna: {} not registered",
                                    hash
                                ))
                            })?;
                        if let Some(props) = properties {
                            let properties = SerializedBytes::try_from(props)
                                .map_err(SerializationError::from)?;
                            dna = dna.with_properties(properties).await?;
                        }
                        if let Some(uid) = uid {
                            dna = dna.with_uid(uid).await?;
                        }
                        dna
                    }
                    DnaSource::Path(ref path) => {
                        let bundle = Bundle::read_from_file(path).await?;
                        let bundle: DnaBundle = bundle.into();
                        let (dna_file, _original_hash) =
                            bundle.into_dna_file(uid, properties).await?;
                        dna_file
                    }
                    DnaSource::Bundle(bundle) => {
                        let (dna_file, _original_hash) =
                            bundle.into_dna_file(uid, properties).await?;
                        dna_file
                    }
                };

                let hash = dna.dna_hash().clone();
                let dna_list = self.conductor_handle.list_dnas().await?;
                if !dna_list.contains(&hash) {
                    self.conductor_handle.register_dna(dna).await?;
                }
                Ok(AdminResponse::DnaRegistered(hash))
            }
            CreateCloneCell(payload) => {
                let cell_id = payload.cell_id();
                self.conductor_handle
                    .clone()
                    .create_clone_cell(*payload)
                    .await?;
                Ok(AdminResponse::CloneCellCreated(cell_id))
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
                        hash,
                        membrane_proof,
                        nick,
                    } = dna_payload;

                    // confirm that hash has been installed
                    let dna_list = self.conductor_handle.list_dnas().await?;
                    if !dna_list.contains(&hash) {
                        return Err(ConductorApiError::DnaReadError(format!(
                            "Given dna has not been registered: {}",
                            hash
                        )));
                    }
                    let cell_id = CellId::from((hash.clone(), agent_key.clone()));
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
                    .install_app(installed_app_id.clone(), cell_ids_with_proofs.clone())
                    .await?;

                let installed_cells = cell_ids_with_proofs
                    .into_iter()
                    .map(|(cell_data, _)| cell_data);
                let app = InstalledApp::new_inactive(InstalledAppCommon::new_legacy(
                    installed_app_id,
                    installed_cells,
                )?);
                let info = InstalledAppInfo::from_installed_app(&app);
                Ok(AdminResponse::AppInstalled(info))
            }
            InstallAppBundle(payload) => {
                let app: InstalledApp = self
                    .conductor_handle
                    .clone()
                    .install_app_bundle(*payload)
                    .await?
                    .into();
                Ok(AdminResponse::AppBundleInstalled(
                    InstalledAppInfo::from_installed_app(&app),
                ))
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
                let app = self
                    .conductor_handle
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
                    .unwrap_or_else(|| {
                        Ok(AdminResponse::AppActivated(
                            InstalledAppInfo::from_installed_app(&InstalledApp::Active(app)),
                        ))
                    })
            }
            DeactivateApp { installed_app_id } => {
                // Activate app
                self.conductor_handle
                    .deactivate_app(installed_app_id.clone(), DeactivationReason::Normal)
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
            ListAppInterfaces => {
                let interfaces = self.conductor_handle.list_app_interfaces().await?;
                Ok(AdminResponse::AppInterfacesListed(interfaces))
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

/// Return the proper phenotype for a Dna, given a manifest and some optional
/// overrides
fn _resolve_phenotype(
    manifest: &DnaManifest,
    payload_uid: Option<&Uid>,
    payload_properties: Option<&YamlProperties>,
) -> (Option<Uid>, Option<YamlProperties>) {
    let bundle_uid = manifest.uid();
    let bundle_properties = manifest.properties();
    let properties = if payload_properties.is_some() {
        payload_properties.cloned()
    } else {
        bundle_properties
    };
    let uid = if payload_uid.is_some() {
        payload_uid.cloned()
    } else {
        bundle_uid
    };
    (uid, properties)
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
    use holochain_types::test_utils::fake_dna_zomes;
    use holochain_types::test_utils::write_fake_dna_file;
    use holochain_wasm_test_utils::TestWasm;
    use matches::assert_matches;
    use observability;
    use uuid::Uuid;

    #[tokio::test(flavor = "multi_thread")]
    async fn register_list_dna_app() -> Result<()> {
        observability::test_run().ok();
        let envs = test_environments();
        let handle = Conductor::builder().test(&envs).await?;
        let shutdown = handle.take_shutdown_handle().await.unwrap();
        let admin_api = RealAdminInterfaceApi::new(handle.clone());
        let uid = Uuid::new_v4();
        let dna = fake_dna_zomes(
            &uid.to_string(),
            vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
        );
        let dna_hash = dna.dna_hash().clone();
        let (dna_path, _tempdir) = write_fake_dna_file(dna.clone()).await.unwrap();
        let path_payload = RegisterDnaPayload {
            uid: None,
            properties: None,
            source: DnaSource::Path(dna_path.clone()),
        };
        let path_install_response = admin_api
            .handle_admin_request(AdminRequest::RegisterDna(Box::new(path_payload)))
            .await;
        assert_matches!(
            path_install_response,
            AdminResponse::DnaRegistered(h) if h == dna_hash
        );

        // re-register idempotent
        let path_payload = RegisterDnaPayload {
            uid: None,
            properties: None,
            source: DnaSource::Path(dna_path.clone()),
        };
        let path1_install_response = admin_api
            .handle_admin_request(AdminRequest::RegisterDna(Box::new(path_payload)))
            .await;
        assert_matches!(
            path1_install_response,
            AdminResponse::DnaRegistered(h) if h == dna_hash
        );

        let dna_list = admin_api.handle_admin_request(AdminRequest::ListDnas).await;
        let expects = vec![dna_hash.clone()];
        assert_matches!(dna_list, AdminResponse::DnasListed(a) if a == expects);

        // register by hash
        let hash_payload = RegisterDnaPayload {
            uid: None,
            properties: None,
            source: DnaSource::Hash(dna_hash.clone()),
        };

        // without properties or uid should throw error
        let hash_install_response = admin_api
            .handle_admin_request(AdminRequest::RegisterDna(Box::new(hash_payload)))
            .await;
        assert_matches!(
            hash_install_response,
            AdminResponse::Error(ExternalApiWireError::DnaReadError(e)) if e == String::from("Hash Dna source requires properties or uid to create a derived Dna")
        );

        // with a property should install and produce a different hash
        let json: serde_yaml::Value = serde_yaml::from_str("some prop: \"foo\"").unwrap();
        let hash_payload = RegisterDnaPayload {
            uid: None,
            properties: Some(YamlProperties::new(json.clone())),
            source: DnaSource::Hash(dna_hash.clone()),
        };
        let install_response = admin_api
            .handle_admin_request(AdminRequest::RegisterDna(Box::new(hash_payload)))
            .await;
        assert_matches!(
            install_response,
            AdminResponse::DnaRegistered(hash) if hash != dna_hash
        );

        // with a uid should install and produce a different hash
        let hash_payload = RegisterDnaPayload {
            uid: Some(String::from("12345678900000000000000")),
            properties: None,
            source: DnaSource::Hash(dna_hash.clone()),
        };
        let hash2_install_response = admin_api
            .handle_admin_request(AdminRequest::RegisterDna(Box::new(hash_payload)))
            .await;

        let new_hash = if let AdminResponse::DnaRegistered(ref h) = hash2_install_response {
            h.clone()
        } else {
            unreachable!()
        };

        assert_matches!(
            hash2_install_response,
            AdminResponse::DnaRegistered(hash) if hash != dna_hash
        );

        // from a path with a same uid should return the already registered hash so it's idempotent
        let path_payload = RegisterDnaPayload {
            uid: Some(String::from("12345678900000000000000")),
            properties: None,
            source: DnaSource::Path(dna_path.clone()),
        };
        let path2_install_response = admin_api
            .handle_admin_request(AdminRequest::RegisterDna(Box::new(path_payload)))
            .await;
        assert_matches!(
            path2_install_response,
            AdminResponse::DnaRegistered(hash) if hash == new_hash
        );

        // from a path with different uid should produce different hash
        let path_payload = RegisterDnaPayload {
            uid: Some(String::from("foo")),
            properties: None,
            source: DnaSource::Path(dna_path),
        };
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

    #[tokio::test(flavor = "multi_thread")]
    async fn install_list_dna_app() -> Result<()> {
        observability::test_run().ok();
        let envs = test_environments();
        let handle = Conductor::builder().test(&envs).await?;
        let shutdown = handle.take_shutdown_handle().await.unwrap();
        let admin_api = RealAdminInterfaceApi::new(handle.clone());
        let uid = Uuid::new_v4();
        let dna = fake_dna_zomes(
            &uid.to_string(),
            vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
        );
        let (dna_path, _tempdir) = write_fake_dna_file(dna.clone()).await.unwrap();
        let agent_key1 = fake_agent_pubkey_1();

        // attempt install with a hash before the DNA has been registered
        let dna_hash = dna.dna_hash().clone();
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

        // now register a DNA
        let path_payload = RegisterDnaPayload {
            uid: None,
            properties: None,
            source: DnaSource::Path(dna_path),
        };
        let path_install_response = admin_api
            .handle_admin_request(AdminRequest::RegisterDna(Box::new(path_payload)))
            .await;
        assert_matches!(
            path_install_response,
            AdminResponse::DnaRegistered(h) if h == dna_hash
        );

        let agent_key2 = fake_agent_pubkey_2();
        let path_payload = InstallAppDnaPayload::hash_only(dna_hash.clone(), "".to_string());
        let cell_id2 = CellId::new(dna_hash.clone(), agent_key2.clone());
        let expected_installed_app = InstalledApp::new_inactive(
            InstalledAppCommon::new_legacy(
                "test-by-path".to_string(),
                vec![InstalledCell::new(cell_id2.clone(), "".to_string())],
            )
            .unwrap(),
        );
        let expected_installed_app_info: InstalledAppInfo = (&expected_installed_app).into();
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
            AdminResponse::AppInstalled(info) if info == expected_installed_app_info
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
}
