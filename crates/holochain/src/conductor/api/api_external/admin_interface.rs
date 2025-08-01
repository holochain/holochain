use crate::conductor::api::error::ConductorApiError;
use crate::conductor::api::error::ConductorApiResult;
use crate::conductor::api::error::SerializationError;
use crate::conductor::error::ConductorError;
use crate::conductor::interface::error::InterfaceError;
use crate::conductor::interface::error::InterfaceResult;
use crate::conductor::ConductorHandle;
use holochain_serialized_bytes::prelude::*;
use holochain_types::dna::DnaBundle;
use holochain_types::prelude::*;
use mr_bundle::FileSystemBundler;
use std::collections::HashSet;
use tracing::*;

pub use holochain_conductor_api::*;

/// The admin interface that external connections
/// can use to make requests to the conductor
/// The concrete (non-mock) implementation of the AdminInterfaceApi
#[derive(Clone)]
pub struct AdminInterfaceApi {
    /// Mutable access to the Conductor
    conductor_handle: ConductorHandle,
}

impl AdminInterfaceApi {
    /// Create an admin interface api.
    pub fn new(conductor_handle: ConductorHandle) -> Self {
        AdminInterfaceApi { conductor_handle }
    }

    /// Handle an [AdminRequest] and return an [AdminResponse].
    pub async fn handle_request(
        &self,
        request: Result<AdminRequest, SerializedBytesError>,
    ) -> InterfaceResult<AdminResponse> {
        // Don't hold the read across both awaits
        {
            self.conductor_handle
                .check_running()
                .map_err(Box::new)
                .map_err(InterfaceError::RequestHandler)?;
        }
        match request {
            Ok(request) => Ok(self.handle_admin_request(request).await),
            Err(e) => Ok(AdminResponse::Error(SerializationError::from(e).into())),
        }
    }

    /// Deal with error cases produced by `handle_admin_request_inner`
    pub(crate) async fn handle_admin_request(&self, request: AdminRequest) -> AdminResponse {
        debug!("admin request: {:?}", request);

        let res = self
            .handle_admin_request_inner(request)
            .await
            .unwrap_or_else(|e| AdminResponse::Error(e.into()));
        debug!("admin response: {:?}", res);
        res
    }

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
                let RegisterDnaPayload { modifiers, source } = *payload;
                let modifiers = modifiers.serialized().map_err(SerializationError::Bytes)?;
                // network seed and properties from the register call will override any in the bundle
                let dna = match source {
                    DnaSource::Hash(ref hash) => {
                        if !modifiers.has_some_option_set() {
                            return Err(ConductorApiError::DnaReadError(
                                "DnaSource::Hash requires `properties` or `network_seed` to create a derived Dna"
                                    .to_string(),
                            ));
                        }
                        self.conductor_handle
                            .get_dna_file(hash)
                            .ok_or_else(|| {
                                ConductorApiError::DnaReadError(format!(
                                    "Unable to create derived Dna: {} not registered",
                                    hash
                                ))
                            })?
                            .update_modifiers(modifiers)
                    }
                    DnaSource::Path(ref path) => {
                        let bundle = FileSystemBundler::load_from::<ValidatedDnaManifest>(path)
                            .await
                            .map(DnaBundle::from)?;
                        let (dna_file, _original_hash) = bundle.into_dna_file(modifiers).await?;
                        dna_file
                    }
                    DnaSource::Bundle(bundle) => {
                        let (dna_file, _original_hash) = bundle.into_dna_file(modifiers).await?;
                        dna_file
                    }
                };

                let hash = dna.dna_hash().clone();
                let dna_list = self.conductor_handle.list_dnas();
                if !dna_list.contains(&hash) {
                    self.conductor_handle.register_dna(dna).await?;
                }
                Ok(AdminResponse::DnaRegistered(hash))
            }
            GetDnaDefinition(dna_hash) => {
                let dna_def = self
                    .conductor_handle
                    .get_dna_def(&dna_hash)
                    .ok_or(ConductorApiError::DnaMissing(*dna_hash))?;
                Ok(AdminResponse::DnaDefinitionReturned(dna_def))
            }
            UpdateCoordinators(payload) => {
                let UpdateCoordinatorsPayload { dna_hash, source } = *payload;
                let (coordinator_zomes, wasms) = match source {
                    CoordinatorSource::Path(ref path) => {
                        let bundle = FileSystemBundler::load_from::<CoordinatorManifest>(path)
                            .await
                            .map(CoordinatorBundle::from)?;
                        bundle.into_zomes().await?
                    }
                    CoordinatorSource::Bundle(bundle) => bundle.into_zomes().await?,
                };

                self.conductor_handle
                    .update_coordinators(&dna_hash, coordinator_zomes, wasms)
                    .await?;

                Ok(AdminResponse::CoordinatorsUpdated)
            }
            InstallApp(payload) => {
                let app: InstalledApp = self
                    .conductor_handle
                    .clone()
                    .install_app_bundle(*payload)
                    .await?;
                let dna_definitions = self.conductor_handle.get_dna_definitions(&app)?;
                Ok(AdminResponse::AppInstalled(AppInfo::from_installed_app(
                    &app,
                    &dna_definitions,
                )))
            }
            UninstallApp {
                installed_app_id,
                force,
            } => {
                self.conductor_handle
                    .clone()
                    .uninstall_app(&installed_app_id, force)
                    .await?;
                Ok(AdminResponse::AppUninstalled)
            }
            ListDnas => {
                let dna_list = self.conductor_handle.list_dnas();
                Ok(AdminResponse::DnasListed(dna_list))
            }
            GenerateAgentPubKey => {
                let agent_pub_key = self
                    .conductor_handle
                    .keystore()
                    .clone()
                    .new_sign_keypair_random()
                    .await?;
                Ok(AdminResponse::AgentPubKeyGenerated(agent_pub_key))
            }
            ListCellIds => {
                let cell_ids = self
                    .conductor_handle
                    .running_cell_ids()
                    .into_iter()
                    .collect();
                Ok(AdminResponse::CellIdsListed(cell_ids))
            }
            ListApps { status_filter } => {
                let apps = self.conductor_handle.list_apps(status_filter).await?;
                Ok(AdminResponse::AppsListed(apps))
            }
            EnableApp { installed_app_id } => {
                // Enable app
                let _ = self
                    .conductor_handle
                    .clone()
                    .enable_app(installed_app_id.clone())
                    .await?;
                let app_info = self
                    .conductor_handle
                    .get_app_info(&installed_app_id)
                    .await?
                    .ok_or(ConductorError::AppNotInstalled(installed_app_id))?;
                Ok(AdminResponse::AppEnabled(app_info))
            }
            DisableApp { installed_app_id } => {
                // Disable app
                self.conductor_handle
                    .clone()
                    .disable_app(installed_app_id, DisabledAppReason::User)
                    .await?;
                Ok(AdminResponse::AppDisabled)
            }
            AttachAppInterface {
                port,
                allowed_origins,
                installed_app_id,
            } => {
                let port = port.unwrap_or(0);
                let port = self
                    .conductor_handle
                    .clone()
                    .add_app_interface(
                        either::Either::Left(port),
                        allowed_origins,
                        installed_app_id,
                    )
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
            DumpConductorState => {
                let state = self.conductor_handle.dump_conductor_state().await?;
                Ok(AdminResponse::ConductorStateDumped(state))
            }
            DumpFullState {
                cell_id,
                dht_ops_cursor,
            } => {
                let state = self
                    .conductor_handle
                    .dump_full_cell_state(&cell_id, dht_ops_cursor)
                    .await?;
                Ok(AdminResponse::FullStateDumped(state))
            }
            DumpNetworkMetrics {
                dna_hash,
                include_dht_summary,
            } => {
                let dump = self
                    .conductor_handle
                    .dump_network_metrics(Kitsune2NetworkMetricsRequest {
                        dna_hash,
                        include_dht_summary,
                    })
                    .await?;
                Ok(AdminResponse::NetworkMetricsDumped(dump))
            }
            DumpNetworkStats => {
                let stats = self.conductor_handle.dump_network_stats().await?;
                Ok(AdminResponse::NetworkStatsDumped(stats))
            }
            AddAgentInfo { agent_infos } => {
                self.conductor_handle.add_agent_infos(agent_infos).await?;
                Ok(AdminResponse::AgentInfoAdded)
            }
            AgentInfo { dna_hashes } => {
                let r = self.conductor_handle.get_agent_infos(dna_hashes).await?;
                let mut encoded = Vec::with_capacity(r.len());
                for info in r {
                    encoded.push(info.encode()?);
                }
                Ok(AdminResponse::AgentInfo(encoded))
            }
            PeerMetaInfo { url, dna_hashes } => {
                let r = self
                    .conductor_handle
                    .peer_meta_info(url, dna_hashes)
                    .await?;
                Ok(AdminResponse::PeerMetaInfo(r))
            }
            GraftRecords {
                cell_id,
                validate,
                records,
            } => {
                self.conductor_handle
                    .clone()
                    .graft_records_onto_source_chain(cell_id, validate, records)
                    .await?;
                Ok(AdminResponse::RecordsGrafted)
            }
            GrantZomeCallCapability(payload) => self
                .conductor_handle
                .grant_zome_call_capability(*payload)
                .await
                .map(AdminResponse::ZomeCallCapabilityGranted),

            RevokeZomeCallCapability {
                action_hash,
                cell_id,
            } => {
                self.conductor_handle
                    .revoke_zome_call_capability(cell_id, action_hash)
                    .await?;
                Ok(AdminResponse::ZomeCallCapabilityRevoked)
            }

            ListCapabilityGrants {
                installed_app_id,
                include_revoked,
            } => {
                let state = self.conductor_handle.clone().get_state().await?;
                let app = state.get_app(&installed_app_id)?;
                let app_cells: HashSet<CellId> = app.required_cells().collect();
                let cap_info = self
                    .conductor_handle
                    .clone()
                    .capability_grant_info(&app_cells, include_revoked)
                    .await?;
                Ok(AdminResponse::CapabilityGrantsInfo(cap_info))
            }

            DeleteCloneCell(payload) => {
                self.conductor_handle.delete_clone_cell(&payload).await?;
                Ok(AdminResponse::CloneCellDeleted)
            }
            StorageInfo => Ok(AdminResponse::StorageInfo(
                self.conductor_handle.storage_info().await?,
            )),
            IssueAppAuthenticationToken(payload) => {
                Ok(AdminResponse::AppAuthenticationTokenIssued(
                    self.conductor_handle
                        .issue_app_authentication_token(payload)?,
                ))
            }
            RevokeAppAuthenticationToken(token) => {
                self.conductor_handle
                    .revoke_app_authentication_token(token)?;
                Ok(AdminResponse::AppAuthenticationTokenRevoked)
            }
            #[cfg(feature = "unstable-migration")]
            GetCompatibleCells(dna_hash) => Ok(AdminResponse::CompatibleCells(
                self.conductor_handle
                    .cells_by_dna_lineage(&dna_hash)
                    .await?,
            )),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::conductor::Conductor;
    use anyhow::Result;
    use holochain_state::prelude::*;
    use holochain_trace;
    use holochain_types::test_utils::fake_dna_zomes;
    use holochain_types::test_utils::write_fake_dna_file;
    use holochain_wasm_test_utils::TestWasm;
    use matches::assert_matches;
    use uuid::Uuid;

    #[tokio::test(flavor = "multi_thread")]
    async fn register_list_dna_app() -> Result<()> {
        holochain_trace::test_run();
        let env_dir = test_db_dir();
        let handle = Conductor::builder()
            .with_data_root_path(env_dir.path().to_path_buf().into())
            .test(&[])
            .await?;

        let admin_api = AdminInterfaceApi::new(handle.clone());
        let network_seed = Uuid::new_v4();
        let dna = fake_dna_zomes(
            &network_seed.to_string(),
            vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
        );
        let dna_hash = dna.dna_hash().clone();
        let (dna_path, _tempdir) = write_fake_dna_file(dna.clone()).await.unwrap();
        let path_payload = RegisterDnaPayload {
            modifiers: DnaModifiersOpt::none(),
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
            modifiers: DnaModifiersOpt::none(),
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
            modifiers: DnaModifiersOpt::none(),
            source: DnaSource::Hash(dna_hash.clone()),
        };

        // without modifiers seed should throw error
        let hash_install_response = admin_api
            .handle_admin_request(AdminRequest::RegisterDna(Box::new(hash_payload)))
            .await;
        assert_matches!(
            hash_install_response,
            AdminResponse::Error(ExternalApiWireError::DnaReadError(e)) if e == *"DnaSource::Hash requires `properties` or `network_seed` to create a derived Dna"
        );

        // with a property should install and produce a different hash
        let json: serde_yaml::Value = serde_yaml::from_str("some prop: \"foo\"").unwrap();
        let hash_payload = RegisterDnaPayload {
            modifiers: DnaModifiersOpt::none().with_properties(YamlProperties::new(json.clone())),
            source: DnaSource::Hash(dna_hash.clone()),
        };
        let install_response = admin_api
            .handle_admin_request(AdminRequest::RegisterDna(Box::new(hash_payload)))
            .await;
        assert_matches!(
            install_response,
            AdminResponse::DnaRegistered(hash) if hash != dna_hash
        );

        // with a network seed should install and produce a different hash
        let hash_payload = RegisterDnaPayload {
            modifiers: DnaModifiersOpt::none()
                .with_network_seed(String::from("12345678900000000000000")),
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

        // from a path with a same network seed should return the already registered hash so it's idempotent
        let path_payload = RegisterDnaPayload {
            modifiers: DnaModifiersOpt::none()
                .with_network_seed(String::from("12345678900000000000000")),
            source: DnaSource::Path(dna_path.clone()),
        };
        let path2_install_response = admin_api
            .handle_admin_request(AdminRequest::RegisterDna(Box::new(path_payload)))
            .await;
        assert_matches!(
            path2_install_response,
            AdminResponse::DnaRegistered(hash) if hash == new_hash
        );

        // from a path with different network seed should produce different hash
        let path_payload = RegisterDnaPayload {
            modifiers: DnaModifiersOpt::none().with_network_seed(String::from("foo")),
            source: DnaSource::Path(dna_path),
        };
        let path3_install_response = admin_api
            .handle_admin_request(AdminRequest::RegisterDna(Box::new(path_payload)))
            .await;
        assert_matches!(
            path3_install_response,
            AdminResponse::DnaRegistered(hash) if hash != dna_hash
        );

        tokio::time::timeout(std::time::Duration::from_secs(1), handle.shutdown())
            .await
            .ok();
        Ok(())
    }

    // @todo fix test by using new InstallApp call
    // #[tokio::test(flavor = "multi_thread")]
    // async fn install_list_dna_app() {
    // holochain_trace::test_run();
    // let db_dir = test_db_dir();
    // let handle = Conductor::builder().test(db_dir.path(), &[]).await.unwrap();
    // let shutdown = handle.take_shutdown_handle().unwrap();
    // let admin_api = RealAdminInterfaceApi::new(handle.clone());
    // let network_seed = Uuid::new_v4();
    // let dna = fake_dna_zomes(
    //     &network_seed.to_string(),
    //     vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
    // );
    // let (dna_path, _tempdir) = write_fake_dna_file(dna.clone()).await.unwrap();
    // let agent_key1 = fake_agent_pubkey_1();

    // attempt install with a hash before the DNA has been registered
    // let dna_hash = dna.dna_hash().clone();
    // let hash_payload = InstallAppDnaPayload::hash_only(dna_hash.clone(), "".to_string());
    // let hash_install_payload = InstallAppPayload {
    //     dnas: vec![hash_payload],
    //     installed_app_id: "test-by-hash".to_string(),
    //     agent_key: agent_key1,
    // };
    // let install_response = admin_api
    //     .handle_admin_request(AdminRequest::InstallApp(Box::new(
    //         hash_install_payload.clone(),
    //     )))
    //     .await;
    // assert_matches!(
    //     install_response,
    //     AdminResponse::Error(ExternalApiWireError::DnaReadError(e)) if e == format!("Given dna has not been registered: {}", dna_hash)
    // );

    // now register a DNA
    // let path_payload = RegisterDnaPayload {
    //     modifiers: DnaModifiersOpt::none(),
    //     source: DnaSource::Path(dna_path),
    // };
    // let path_install_response = admin_api
    //     .handle_admin_request(AdminRequest::RegisterDna(Box::new(path_payload)))
    //     .await;
    // assert_matches!(
    //     path_install_response,
    //     AdminResponse::DnaRegistered(h) if h == dna_hash
    // );

    // let agent_key2 = fake_agent_pubkey_2();
    // let path_payload = InstallAppDnaPayload::hash_only(dna_hash.clone(), "".to_string());
    // let cell_id2 = CellId::new(dna_hash.clone(), agent_key2.clone());
    // let expected_installed_app = InstalledApp::new_fresh(
    //     InstalledAppCommon::new_legacy(
    //         "test-by-path".to_string(),
    //         vec![InstalledCell::new(cell_id2.clone(), "".to_string())],
    //     )
    //     .unwrap(),
    // );
    // let expected_installed_app_info: InstalledAppInfo = (&expected_installed_app).into();
    // let path_install_payload = InstallAppPayload {
    //     dnas: vec![path_payload],
    //     installed_app_id: "test-by-path".to_string(),
    //     agent_key: agent_key2,
    // };

    // let install_response = admin_api
    //     .handle_admin_request(AdminRequest::InstallApp(Box::new(path_install_payload)))
    //     .await;
    // assert_matches!(
    //     install_response,
    //     AdminResponse::AppInstalled(info) if info == expected_installed_app_info
    // );
    // let dna_list = admin_api.handle_admin_request(AdminRequest::ListDnas).await;
    // let expects = vec![dna_hash.clone()];
    // assert_matches!(dna_list, AdminResponse::DnasListed(a) if a == expects);

    // let expected_enabled_app = InstalledApp::new_running(
    //     InstalledAppCommon::new_legacy(
    //         "test-by-path".to_string(),
    //         vec![InstalledCell::new(cell_id2.clone(), "".to_string())],
    //     )
    //     .unwrap(),
    // );
    // let expected_enabled_app_info: InstalledAppInfo = (&expected_enabled_app).into();
    // let res = admin_api
    //     .handle_admin_request(AdminRequest::EnableApp {
    //         installed_app_id: "test-by-path".to_string(),
    //     })
    //     .await;
    // assert_matches!(res,
    //     AdminResponse::AppEnabled {app, ..} if app == expected_enabled_app_info
    // );

    // let res = admin_api
    //     .handle_admin_request(AdminRequest::ListCellIds)
    //     .await;

    // assert_matches!(res, AdminResponse::CellIdsListed(v) if v == vec![cell_id2]);

    // now try to install the happ using the hash
    // let _install_response = admin_api
    //     .handle_admin_request(AdminRequest::InstallApp(Box::new(hash_install_payload)))
    //     .await;
    // let _res = admin_api
    //     .handle_admin_request(AdminRequest::EnableApp {
    //         installed_app_id: "test-by-hash".to_string(),
    //     })
    //     .await;

    // let res = admin_api
    //     .handle_admin_request(AdminRequest::ListApps {
    //         status_filter: Some(AppStatusFilter::Enabled),
    //     })
    //     .await;

    // assert_matches!(res, AdminResponse::AppsListed(v) if v.iter().find(|app_info| app_info.installed_app_id.as_str() == "test-by-path").is_some() && v.iter().find(|app_info| app_info.installed_app_id.as_str() == "test-by-hash").is_some());

    // handle.shutdown();
    // tokio::time::timeout(std::time::Duration::from_secs(1), shutdown)
    //     .await
    //     .ok();
    // }
}
