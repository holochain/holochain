use crate::conductor::api::error::ConductorApiError;
use crate::conductor::api::error::ConductorApiResult;
use crate::conductor::api::error::SerializationError;
use crate::conductor::error::ConductorError;
use crate::conductor::interface::error::InterfaceError;
use crate::conductor::interface::error::InterfaceResult;
use crate::conductor::ConductorHandle;
pub use holochain_conductor_api::*;
use holochain_serialized_bytes::prelude::*;
use holochain_types::prelude::*;
use mr_bundle::FileSystemBundler;
use std::collections::HashSet;
use tracing::*;

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
            GetDnaDefinition(cell_id) => {
                let dna_def = self
                    .conductor_handle
                    .get_dna_def(&cell_id)
                    .ok_or(ConductorApiError::CellMissing(*cell_id))?;
                Ok(AdminResponse::DnaDefinitionReturned(dna_def))
            }
            UpdateCoordinators(payload) => {
                let UpdateCoordinatorsPayload { cell_id, source } = *payload;
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
                    .update_coordinators(cell_id, coordinator_zomes, wasms)
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
                let dna_list = self.conductor_handle.list_dna_hashes();
                Ok(AdminResponse::DnasListed(dna_list.into_iter().collect()))
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
