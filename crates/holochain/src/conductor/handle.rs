//! Defines [ConductorHandle], a lightweight cloneable reference to a Conductor
//! with a limited public interface.
//!
//! A ConductorHandle can be produced via [Conductor::into_handle]
//!
//! ```rust, no_run
//! async fn async_main () {
//! use holochain_state::test_utils::test_environments;
//! use holochain::conductor::{Conductor, ConductorBuilder, ConductorHandle};
//! let envs = test_environments();
//! let handle: ConductorHandle = ConductorBuilder::new()
//!    .test(&envs)
//!    .await
//!    .unwrap();
//!
//! // handles are cloneable
//! let handle2 = handle.clone();
//!
//! assert_eq!(handle.list_dnas().await.unwrap(), vec![]);
//! handle.shutdown().await;
//!
//! // handle2 will only get errors from now on, since the other handle
//! // shut down the conductor.
//! assert!(handle2.list_dnas().await.is_err());
//!
//! # }
//! ```
//!
//! The purpose of this handle is twofold:
//!
//! First, it specifies how to synchronize
//! read/write access to a single Conductor across multiple references. The various
//! Conductor APIs - [CellConductorApi], [AdminInterfaceApi], and [AppInterfaceApi],
//! use a ConductorHandle as their sole method of interaction with a Conductor.
//!
//! Secondly, it hides the concrete type of the Conductor behind a dyn Trait.
//! The Conductor is a central point of configuration, and has several
//! type parameters, used to modify functionality including specifying mock
//! types for testing. If we did not have a way of hiding this type genericity,
//! code which interacted with the Conductor would also have to be highly generic.

use super::api::error::ConductorApiResult;
use super::api::ZomeCall;
use super::config::AdminInterfaceConfig;
use super::error::ConductorResult;
use super::error::CreateAppError;
use super::interface::SignalBroadcaster;
use super::manager::TaskManagerRunHandle;
use super::p2p_agent_store::get_agent_info_signed;
use super::p2p_agent_store::put_agent_info_signed;
use super::p2p_agent_store::query_agent_info_signed;
use super::p2p_agent_store::query_agent_info_signed_near_basis;
use super::Cell;
use super::Conductor;
use crate::conductor::p2p_metrics::put_metric_datum;
use crate::conductor::p2p_metrics::query_metrics;
use crate::core::workflow::ZomeCallResult;
use crate::core::{queue_consumer::InitialQueueTriggers, ribosome::real_ribosome::RealRibosome};
use derive_more::From;
use futures::future::FutureExt;
use futures::StreamExt;
use holochain_conductor_api::AppStatusFilter;
use holochain_conductor_api::InstalledAppInfo;
use holochain_p2p::event::HolochainP2pEvent;
use holochain_p2p::event::HolochainP2pEvent::*;
use holochain_p2p::DnaHashExt;
use holochain_p2p::HolochainP2pCellT;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_types::prelude::*;
use kitsune_p2p::agent_store::AgentInfoSigned;
use kitsune_p2p::KitsuneSpace;
use kitsune_p2p_types::config::JOIN_NETWORK_TIMEOUT;
use std::{collections::HashSet, sync::Arc};
use tokio::sync::RwLock;
use tracing::*;

#[cfg(any(test, feature = "test_utils"))]
use super::state::ConductorState;
#[cfg(any(test, feature = "test_utils"))]
use crate::core::queue_consumer::QueueTriggers;

/// A handle to the Conductor that can easily be passed around and cheaply cloned
pub type ConductorHandle = Arc<dyn ConductorHandleT>;

/// Base trait for ConductorHandle
#[mockall::automock]
#[async_trait::async_trait]
pub trait ConductorHandleT: Send + Sync {
    /// Returns error if conductor is shutting down
    async fn check_running(&self) -> ConductorResult<()>;

    /// Initialize the task manager, add admin interfaces from config,
    /// start up app interfaces from db, and register all tasks.
    ///
    /// This requires a concrete ConductorHandle to be passed into the
    /// interface tasks. This is a bit weird to do, but it was the only way
    /// around having a circular reference in the types.
    ///
    /// Never use a ConductorHandle for different Conductor here!
    async fn initialize_conductor(
        self: Arc<Self>,
        admin_configs: Vec<AdminInterfaceConfig>,
    ) -> ConductorResult<()>;

    /// Add a collection of admin interfaces from config
    async fn add_admin_interfaces(
        self: Arc<Self>,
        configs: Vec<AdminInterfaceConfig>,
    ) -> ConductorResult<()>;

    /// Add an app interface
    async fn add_app_interface(self: Arc<Self>, port: u16) -> ConductorResult<u16>;

    /// List the app interfaces currently install.
    async fn list_app_interfaces(&self) -> ConductorResult<Vec<u16>>;

    /// Install a [Dna] in this Conductor
    async fn register_dna(&self, dna: DnaFile) -> ConductorResult<()>;

    /// Install just the "code parts" (the wasm and entry defs) of a dna
    async fn register_genotype(&self, dna: DnaFile) -> ConductorResult<()>;

    /// Get the list of hashes of installed Dnas in this Conductor
    async fn list_dnas(&self) -> ConductorResult<Vec<DnaHash>>;

    /// Get a [Dna] from the [DnaStore]
    async fn get_dna(&self, hash: &DnaHash) -> Option<DnaFile>;

    /// Get an instance of a [RealRibosome] for the DnaHash
    async fn get_ribosome(&self, dna_hash: &DnaHash) -> ConductorResult<RealRibosome>;

    /// Get a [EntryDef] from the [EntryDefBuffer]
    async fn get_entry_def(&self, key: &EntryDefBufferKey) -> Option<EntryDef>;

    /// Add the [DnaFile]s from the wasm and dna_def databases into memory
    async fn load_dnas(&self) -> ConductorResult<()>;

    /// Dispatch a network event to the correct cell.
    /// Warning: returning an error from this function kills the network for the conductor.
    async fn dispatch_holochain_p2p_event(
        &self,
        event: holochain_p2p::event::HolochainP2pEvent,
    ) -> ConductorApiResult<()>;

    /// Invoke a zome function on a Cell
    async fn call_zome(&self, invocation: ZomeCall) -> ConductorApiResult<ZomeCallResult>;

    /// Invoke a zome function on a Cell with a workspace
    async fn call_zome_with_workspace(
        &self,
        invocation: ZomeCall,
        workspace_lock: HostFnWorkspace,
    ) -> ConductorApiResult<ZomeCallResult>;

    /// Get a Websocket port which will
    async fn get_arbitrary_admin_websocket_port(&self) -> Option<u16>;

    /// Return the JoinHandle for all managed tasks, which when resolved will
    /// signal that the Conductor has completely shut down.
    ///
    /// NB: The JoinHandle is not cloneable,
    /// so this can only ever be called successfully once.
    async fn take_shutdown_handle(&self) -> Option<TaskManagerRunHandle>;

    /// Send a signal to all managed tasks asking them to end ASAP.
    async fn shutdown(&self);

    /// Request access to this conductor's keystore
    fn keystore(&self) -> &KeystoreSender;

    /// Request access to this conductor's networking handle
    fn holochain_p2p(&self) -> &holochain_p2p::HolochainP2pRef;

    /// Create a new Cell in an existing App based on an existing DNA
    async fn create_clone_cell(
        self: Arc<Self>,
        payload: CreateCloneCellPayload,
    ) -> ConductorResult<CellId>;

    /// Destroy a cloned Cell
    async fn destroy_clone_cell(self: Arc<Self>, cell_id: CellId) -> ConductorResult<()>;

    /// Install Cells into ConductorState based on installation info, and run
    /// genesis on all new source chains
    async fn install_app(
        self: Arc<Self>,
        installed_app_id: InstalledAppId,
        cell_data_with_proofs: Vec<(InstalledCell, Option<MembraneProof>)>,
    ) -> ConductorResult<()>;

    /// Install DNAs and set up Cells as specified by an AppBundle
    async fn install_app_bundle(
        self: Arc<Self>,
        payload: InstallAppBundlePayload,
    ) -> ConductorResult<InactiveApp>;

    /// Uninstall an app from the state DB and remove all running Cells
    async fn uninstall_app(&self, app: &InstalledAppId) -> ConductorResult<()>;

    /// Setup the cells from the database
    /// Only creates any cells that are not already created
    async fn setup_cells(self: Arc<Self>) -> ConductorResult<Vec<CreateAppError>>;

    /// Activate an app
    async fn activate_app(&self, installed_app_id: InstalledAppId) -> ConductorResult<ActiveApp>;

    /// Deactivate an app
    async fn deactivate_app(
        &self,
        installed_app_id: InstalledAppId,
        reason: DeactivationReason,
    ) -> ConductorResult<()>;

    /// List Cell Ids
    async fn list_cell_ids(&self) -> ConductorResult<Vec<CellId>>;

    /// List Active AppIds
    async fn list_active_apps(&self) -> ConductorResult<Vec<InstalledAppId>>;

    /// List Apps with their information
    async fn list_apps(
        &self,
        status_filter: Option<AppStatusFilter>,
    ) -> ConductorResult<Vec<InstalledAppInfo>>;

    /// Get the IDs of all active installed Apps which use this Cell
    async fn list_active_apps_for_cell_id(
        &self,
        cell_id: &CellId,
    ) -> ConductorResult<HashSet<InstalledAppId>>;

    /// Dump the cells state
    async fn dump_cell_state(&self, cell_id: &CellId) -> ConductorApiResult<String>;

    /// Access the broadcast Sender which will send a Signal across every
    /// attached app interface
    async fn signal_broadcaster(&self) -> SignalBroadcaster;

    /// Get info about an installed App, whether active or inactive
    async fn get_app_info(
        &self,
        installed_app_id: &InstalledAppId,
    ) -> ConductorResult<Option<InstalledAppInfo>>;

    /// Add signed agent info to the conductor
    async fn add_agent_infos(&self, agent_infos: Vec<AgentInfoSigned>) -> ConductorApiResult<()>;

    /// Get signed agent info from the conductor
    async fn get_agent_infos(
        &self,
        cell_id: Option<CellId>,
    ) -> ConductorApiResult<Vec<AgentInfoSigned>>;

    /// Print the current setup in a machine readable way.
    async fn print_setup(&self);

    /// Retrieve the environment for this cell.
    async fn get_cell_env_readonly(&self, cell_id: &CellId) -> ConductorApiResult<EnvRead>;

    /// Retrieve the environment for this cell. FOR TESTING ONLY.
    #[cfg(any(test, feature = "test_utils"))]
    async fn get_cell_env(&self, cell_id: &CellId) -> ConductorApiResult<EnvWrite>;

    /// Retrieve the database for this cell. FOR TESTING ONLY.
    #[cfg(any(test, feature = "test_utils"))]
    async fn get_cache_env(&self, cell_id: &CellId) -> ConductorApiResult<EnvWrite>;

    /// Retrieve the database for networking. FOR TESTING ONLY.
    #[cfg(any(test, feature = "test_utils"))]
    async fn get_p2p_env(&self, space: Arc<KitsuneSpace>) -> EnvWrite;

    /// Retrieve Senders for triggering workflows. FOR TESTING ONLY.
    #[cfg(any(test, feature = "test_utils"))]
    async fn get_cell_triggers(&self, cell_id: &CellId) -> ConductorApiResult<QueueTriggers>;

    /// Retrieve the ConductorState. FOR TESTING ONLY.
    #[cfg(any(test, feature = "test_utils"))]
    async fn get_state_from_handle(&self) -> ConductorApiResult<ConductorState>;

    /// Add a "test" app interface for sending and receiving signals. FOR TESTING ONLY.
    #[cfg(any(test, feature = "test_utils"))]
    async fn add_test_app_interface(&self, id: super::state::AppInterfaceId)
        -> ConductorResult<()>;

    #[cfg(any(test, feature = "test_utils"))]
    /// Check whether this conductor should skip gossip.
    fn should_skip_publish(&self) -> bool;

    #[cfg(any(test, feature = "test_utils"))]
    /// For testing we can choose to skip publish.
    fn set_skip_publish(&self, skip_publish: bool);
}

/// The current "production" implementation of a ConductorHandle.
/// The implementation specifies how read/write access to the Conductor
/// should be synchronized across multiple concurrent Handles.
///
/// Synchronization is currently achieved via a simple RwLock, but
/// this could be swapped out with, e.g. a channel Sender/Receiver pair
/// using an actor model.
#[derive(From)]
pub struct ConductorHandleImpl<DS: DnaStore + 'static> {
    pub(crate) conductor: RwLock<Conductor<DS>>,
    pub(crate) keystore: KeystoreSender,
    pub(crate) holochain_p2p: holochain_p2p::HolochainP2pRef,

    // Testing:
    #[cfg(any(test, feature = "test_utils"))]
    /// All conductors should skip publishing.
    /// This is useful for testing gossip.
    pub skip_publish: std::sync::atomic::AtomicBool,
}

#[async_trait::async_trait]
impl<DS: DnaStore + 'static> ConductorHandleT for ConductorHandleImpl<DS> {
    /// Check that shutdown has not been called
    async fn check_running(&self) -> ConductorResult<()> {
        self.conductor.read().await.check_running()
    }

    async fn add_admin_interfaces(
        self: Arc<Self>,
        configs: Vec<AdminInterfaceConfig>,
    ) -> ConductorResult<()> {
        let mut lock = self.conductor.write().await;
        lock.add_admin_interfaces_via_handle(configs, self.clone())
            .await
    }

    async fn initialize_conductor(
        self: Arc<Self>,
        admin_configs: Vec<AdminInterfaceConfig>,
    ) -> ConductorResult<()> {
        let mut conductor = self.conductor.write().await;
        conductor.start_task_manager(self.clone()).await?;
        conductor
            .add_admin_interfaces_via_handle(admin_configs, self.clone())
            .await?;
        conductor
            .startup_app_interfaces_via_handle(self.clone())
            .await?;
        Ok(())
    }

    async fn add_app_interface(self: Arc<Self>, port: u16) -> ConductorResult<u16> {
        let mut lock = self.conductor.write().await;
        lock.add_app_interface_via_handle(either::Left(port), self.clone())
            .await
    }

    async fn list_app_interfaces(&self) -> ConductorResult<Vec<u16>> {
        self.conductor.read().await.list_app_interfaces().await
    }

    async fn register_dna(&self, dna: DnaFile) -> ConductorResult<()> {
        self.register_genotype(dna.clone()).await?;
        self.conductor.write().await.register_phenotype(dna).await
    }

    async fn register_genotype(&self, dna: DnaFile) -> ConductorResult<()> {
        let entry_defs = self.conductor.read().await.register_dna_wasm(dna).await?;
        self.conductor
            .write()
            .await
            .register_dna_entry_defs(entry_defs)
            .await?;
        Ok(())
    }

    async fn load_dnas(&self) -> ConductorResult<()> {
        let (dnas, entry_defs) = self
            .conductor
            .read()
            .await
            .load_wasms_into_dna_files()
            .await?;
        let mut lock = self.conductor.write().await;
        lock.dna_store_mut().add_dnas(dnas);
        lock.dna_store_mut().add_entry_defs(entry_defs);
        Ok(())
    }

    async fn list_dnas(&self) -> ConductorResult<Vec<DnaHash>> {
        Ok(self.conductor.read().await.dna_store().list())
    }

    async fn get_dna(&self, hash: &DnaHash) -> Option<DnaFile> {
        self.conductor.read().await.dna_store().get(hash)
    }

    async fn get_ribosome(&self, dna_hash: &DnaHash) -> ConductorResult<RealRibosome> {
        self.conductor.read().await.get_ribosome(dna_hash)
    }

    async fn get_entry_def(&self, key: &EntryDefBufferKey) -> Option<EntryDef> {
        self.conductor.read().await.dna_store().get_entry_def(key)
    }

    #[instrument(skip(self))]
    async fn dispatch_holochain_p2p_event(
        &self,
        event: holochain_p2p::event::HolochainP2pEvent,
    ) -> ConductorApiResult<()> {
        let space = event.dna_hash().to_kitsune();
        trace!(dispatch_event = ?event);
        match event {
            PutAgentInfoSigned {
                agent_info_signed,
                respond,
                ..
            } => {
                // TODO: This read lock isn't needed to get the p2p_env.
                let env = { self.conductor.read().await.p2p_env(space) };
                let res = put_agent_info_signed(env, agent_info_signed)
                    .await
                    .map_err(holochain_p2p::HolochainP2pError::other);
                respond.respond(Ok(async move { res }.boxed().into()));
            }
            GetAgentInfoSigned {
                kitsune_space,
                kitsune_agent,
                respond,
                ..
            } => {
                let env = { self.conductor.read().await.p2p_env(space) };
                let res = get_agent_info_signed(env, kitsune_space, kitsune_agent)
                    .map_err(holochain_p2p::HolochainP2pError::other);
                respond.respond(Ok(async move { res }.boxed().into()));
            }
            QueryAgentInfoSigned {
                kitsune_space,
                respond,
                ..
            } => {
                let env = { self.conductor.read().await.p2p_env(space) };
                let res = query_agent_info_signed(env, kitsune_space)
                    .map_err(holochain_p2p::HolochainP2pError::other);
                respond.respond(Ok(async move { res }.boxed().into()));
            }
            QueryGossipAgents {
                since_ms,
                until_ms,
                arc_set,
                respond,
                ..
            } => {
                use holochain_sqlite::db::AsP2pAgentStoreConExt;
                let env = { self.conductor.read().await.p2p_env(space) };
                let res = env
                    .conn()?
                    .p2p_gossip_query_agents(since_ms, until_ms, (*arc_set).clone())
                    // FIXME: This sucks we have to iterate through the whole vec just to add Arcs.
                    // Are arcs really saving us that much?
                    .map(|r| {
                        r.into_iter()
                            .map(|(agent, arc)| (Arc::new(agent), arc))
                            .collect()
                    })
                    .map_err(holochain_p2p::HolochainP2pError::other);
                respond.respond(Ok(async move { res }.boxed().into()));
            }
            QueryAgentInfoSignedNearBasis {
                kitsune_space,
                basis_loc,
                limit,
                respond,
                ..
            } => {
                let env = { self.conductor.read().await.p2p_env(space) };
                let res = query_agent_info_signed_near_basis(env, kitsune_space, basis_loc, limit)
                    .map_err(holochain_p2p::HolochainP2pError::other);
                respond.respond(Ok(async move { res }.boxed().into()));
            }
            PutMetricDatum {
                respond,
                agent,
                metric,
                timestamp,
                ..
            } => {
                let env = { self.conductor.read().await.p2p_metrics_env(space) };
                let res = put_metric_datum(env, agent, metric, timestamp)
                    .await
                    .map_err(holochain_p2p::HolochainP2pError::other);
                respond.respond(Ok(async move { res }.boxed().into()));
            }
            QueryMetrics { respond, query, .. } => {
                let env = { self.conductor.read().await.p2p_metrics_env(space) };
                let res = query_metrics(env, query)
                    .await
                    .map_err(holochain_p2p::HolochainP2pError::other);
                respond.respond(Ok(async move { res }.boxed().into()));
            }
            SignNetworkData {
                respond,
                to_agent,
                data,
                ..
            } => {
                let signature = to_agent.sign_raw(self.keystore(), &data).await?;
                respond.respond(Ok(async move { Ok(signature) }.boxed().into()));
            }
            HolochainP2pEvent::CallRemote { .. }
            | Publish { .. }
            | GetValidationPackage { .. }
            | Get { .. }
            | GetMeta { .. }
            | GetLinks { .. }
            | GetAgentActivity { .. }
            | ValidationReceiptReceived { .. }
            | FetchOpHashesForConstraints { .. }
            | FetchOpHashData { .. } => {
                let cell_id = CellId::new(
                    event.dna_hash().clone(),
                    event.target_agent_as_ref().clone(),
                );
                let cell = self.cell_by_id(&cell_id).await?;
                cell.handle_holochain_p2p_event(event).await?;
            }
            HashesForTimeWindow {
                to_agents,
                dna_hash,
                window,
                max_ops,
                respond,
                ..
            } => {
                let mut hashes_and_times = Vec::with_capacity(to_agents.len());

                // For each cell collect the hashes and times that fit within the
                // agents interval and time window.
                for (agent, arc_set) in to_agents {
                    let cell_id = CellId::new(dna_hash.clone(), agent);
                    let cell = self.cell_by_id(&cell_id).await?;
                    match cell
                        .handle_hashes_for_time_window(arc_set, window.clone())
                        .await
                    {
                        Ok(t) => hashes_and_times.extend(t),
                        Err(e) => {
                            // If there's an error for any cell we want to fail the whole call.
                            respond.respond(Ok(async move {
                                Err(holochain_p2p::HolochainP2pError::other(e))
                            }
                            .boxed()
                            .into()));
                            return Ok(());
                        }
                    }
                }
                // Remove any duplicate hashes.
                // Note vec must be sorted to remove duplicates.
                hashes_and_times.sort_unstable_by(|a, b| a.0.cmp(&b.0));
                hashes_and_times.dedup_by(|a, b| a.0 == b.0);

                // Now sort by time so we can take up to max_ops.
                hashes_and_times.sort_unstable_by_key(|(_, t)| *t);

                // The start time bound if there is one.
                let start = hashes_and_times.first().map(|(_, t)| *t);

                // The end time bound if there is one.
                let end = hashes_and_times
                    .iter()
                    .take(max_ops)
                    .last()
                    .map(|(_, t)| *t);

                // Extract the hashes.
                let hashes = hashes_and_times
                    .into_iter()
                    .map(|(h, _)| h)
                    .take(max_ops)
                    .collect();

                // The range is exclusive so we add one to the end.
                let range =
                    start.and_then(|s| end.map(|e| (hashes, s..(e.checked_add(1).unwrap_or(0)))));

                respond.respond(Ok(async move { Ok(range) }.boxed().into()));
            }
        }
        Ok(())
    }

    async fn call_zome(&self, call: ZomeCall) -> ConductorApiResult<ZomeCallResult> {
        let cell = self.cell_by_id(&call.cell_id).await?;
        Ok(cell.call_zome(call, None).await?)
    }

    async fn call_zome_with_workspace(
        &self,
        call: ZomeCall,
        workspace_lock: HostFnWorkspace,
    ) -> ConductorApiResult<ZomeCallResult> {
        debug!(cell_id = ?call.cell_id);
        let cell = self.cell_by_id(&call.cell_id).await?;
        Ok(cell.call_zome(call, Some(workspace_lock)).await?)
    }

    async fn take_shutdown_handle(&self) -> Option<TaskManagerRunHandle> {
        self.conductor.write().await.take_shutdown_handle()
    }

    async fn get_arbitrary_admin_websocket_port(&self) -> Option<u16> {
        self.conductor
            .read()
            .await
            .get_arbitrary_admin_websocket_port()
    }

    async fn shutdown(&self) {
        self.conductor.write().await.shutdown()
    }

    fn keystore(&self) -> &KeystoreSender {
        &self.keystore
    }

    fn holochain_p2p(&self) -> &holochain_p2p::HolochainP2pRef {
        &self.holochain_p2p
    }

    async fn create_clone_cell(
        self: Arc<Self>,
        payload: CreateCloneCellPayload,
    ) -> ConductorResult<CellId> {
        let CreateCloneCellPayload {
            properties,
            dna_hash,
            installed_app_id,
            agent_key,
            slot_id,
            membrane_proof,
        } = payload;
        {
            let conductor = self.conductor.read().await;
            let cell_id = CellId::new(dna_hash, agent_key);
            let cells = vec![(cell_id.clone(), membrane_proof)];
            conductor.genesis_cells(cells, self.clone()).await?;
        }
        {
            let mut conductor = self.conductor.write().await;
            let properties = properties.unwrap_or_else(|| ().into());
            let cell_id = conductor
                .add_clone_cell_to_app(&installed_app_id, &slot_id, properties)
                .await?;
            Ok(cell_id)
        }
    }

    async fn destroy_clone_cell(self: Arc<Self>, _cell_id: CellId) -> ConductorResult<()> {
        todo!()
    }

    async fn install_app(
        self: Arc<Self>,
        installed_app_id: InstalledAppId,
        cell_data: Vec<(InstalledCell, Option<MembraneProof>)>,
    ) -> ConductorResult<()> {
        self.conductor
            .read()
            .await
            .genesis_cells(
                cell_data
                    .iter()
                    .map(|(c, p)| (c.as_id().clone(), p.clone()))
                    .collect(),
                self.clone(),
            )
            .await?;

        let cell_data = cell_data.into_iter().map(|(c, _)| c);
        let app = InstalledAppCommon::new_legacy(installed_app_id, cell_data)?;

        // Update the db
        let _ = self
            .conductor
            .write()
            .await
            .add_inactive_app_to_db(app)
            .await?;

        Ok(())
    }

    async fn install_app_bundle(
        self: Arc<Self>,
        payload: InstallAppBundlePayload,
    ) -> ConductorResult<InactiveApp> {
        let InstallAppBundlePayload {
            source,
            agent_key,
            installed_app_id,
            membrane_proofs,
            uid,
        } = payload;

        let bundle: AppBundle = {
            let original_bundle = source.resolve().await?;
            if let Some(uid) = uid {
                let mut manifest = original_bundle.manifest().to_owned();
                manifest.set_uid(uid);
                AppBundle::from(original_bundle.into_inner().update_manifest(manifest)?)
            } else {
                original_bundle
            }
        };

        let installed_app_id =
            installed_app_id.unwrap_or_else(|| bundle.manifest().app_name().to_owned());
        let ops = bundle
            .resolve_cells(agent_key.clone(), DnaGamut::placeholder(), membrane_proofs)
            .await?;

        let cells_to_create = ops.cells_to_create();

        for (dna, _) in ops.dnas_to_register {
            self.clone().register_dna(dna).await?;
        }

        self.conductor
            .read()
            .await
            .genesis_cells(cells_to_create, self.clone())
            .await?;

        let slots = ops.slots;
        let app = InstalledAppCommon::new(installed_app_id, agent_key, slots);

        // Update the db
        let app = self
            .conductor
            .write()
            .await
            .add_inactive_app_to_db(app.clone())
            .await?;

        Ok(app)
    }

    async fn setup_cells(self: Arc<Self>) -> ConductorResult<Vec<CreateAppError>> {
        let cells = {
            let lock = self.conductor.read().await;
            lock.create_active_app_cells(self.clone())
                .await?
                .into_iter()
        };
        let add_cells_tasks = cells.map(|result| async {
            match result {
                Ok(cells) => {
                    self.initialize_cells(cells).await;
                    None
                }
                Err(e) => Some(e),
            }
        });
        let r = futures::future::join_all(add_cells_tasks)
            .await
            .into_iter()
            .flatten()
            .collect();
        Ok(r)
    }

    async fn activate_app(&self, installed_app_id: InstalledAppId) -> ConductorResult<ActiveApp> {
        let app = self
            .conductor
            .write()
            .await
            .activate_app_in_db(installed_app_id)
            .await?;
        Ok(app)
        // MD: Should we be doing `Conductor::add_cells()` here? (see below comment)
    }

    async fn deactivate_app(
        &self,
        installed_app_id: InstalledAppId,
        reason: DeactivationReason,
    ) -> ConductorResult<()> {
        let mut conductor = self.conductor.write().await;
        let cell_ids_to_remove = conductor
            .deactivate_app_in_db(installed_app_id, reason)
            .await?;
        // MD: I'm not sure about this. We never add the cells back in after re-activating an app,
        //     so it seems either we shouldn't remove them here, or we should be sure to add them
        //     back in when re-activating.
        conductor.remove_cells(cell_ids_to_remove).await;
        Ok(())
    }

    async fn uninstall_app(&self, installed_app_id: &InstalledAppId) -> ConductorResult<()> {
        let mut conductor = self.conductor.write().await;
        if let Some(cell_ids_to_remove) = conductor.remove_app_from_db(installed_app_id).await? {
            conductor.remove_cells(cell_ids_to_remove).await;
        }
        Ok(())
    }

    async fn list_cell_ids(&self) -> ConductorResult<Vec<CellId>> {
        self.conductor.read().await.list_cell_ids().await
    }

    async fn list_active_apps(&self) -> ConductorResult<Vec<InstalledAppId>> {
        self.conductor.read().await.list_active_apps().await
    }

    async fn list_apps(
        &self,
        status_filter: Option<AppStatusFilter>,
    ) -> ConductorResult<Vec<InstalledAppInfo>> {
        self.conductor.read().await.list_apps(status_filter).await
    }

    async fn list_active_apps_for_cell_id(
        &self,
        cell_id: &CellId,
    ) -> ConductorResult<HashSet<InstalledAppId>> {
        self.conductor
            .read()
            .await
            .list_active_apps_for_cell_id(cell_id)
            .await
    }

    async fn dump_cell_state(&self, cell_id: &CellId) -> ConductorApiResult<String> {
        self.conductor.read().await.dump_cell_state(cell_id).await
    }

    async fn signal_broadcaster(&self) -> SignalBroadcaster {
        self.conductor.read().await.signal_broadcaster()
    }

    async fn get_app_info(
        &self,
        installed_app_id: &InstalledAppId,
    ) -> ConductorResult<Option<InstalledAppInfo>> {
        Ok(self
            .conductor
            .read()
            .await
            .get_state()
            .await?
            .get_app_info(installed_app_id))
    }

    async fn add_agent_infos(&self, agent_infos: Vec<AgentInfoSigned>) -> ConductorApiResult<()> {
        self.conductor
            .read()
            .await
            .add_agent_infos(agent_infos)
            .await
    }

    async fn get_agent_infos(
        &self,
        cell_id: Option<CellId>,
    ) -> ConductorApiResult<Vec<AgentInfoSigned>> {
        self.conductor.read().await.get_agent_infos(cell_id)
    }

    async fn print_setup(&self) {
        self.conductor.read().await.print_setup()
    }

    async fn get_cell_env_readonly(&self, cell_id: &CellId) -> ConductorApiResult<EnvRead> {
        let cell = self.cell_by_id(cell_id).await?;
        Ok(cell.env().clone().into())
    }

    #[cfg(any(test, feature = "test_utils"))]
    async fn get_cell_env(&self, cell_id: &CellId) -> ConductorApiResult<EnvWrite> {
        let cell = self.cell_by_id(cell_id).await?;
        Ok(cell.env().clone())
    }

    #[cfg(any(test, feature = "test_utils"))]
    async fn get_cache_env(&self, cell_id: &CellId) -> ConductorApiResult<EnvWrite> {
        let cell = self.cell_by_id(cell_id).await?;
        Ok(cell.cache().clone())
    }

    #[cfg(any(test, feature = "test_utils"))]
    async fn get_p2p_env(&self, space: Arc<KitsuneSpace>) -> EnvWrite {
        let lock = self.conductor.read().await;
        lock.p2p_env(space)
    }

    #[cfg(any(test, feature = "test_utils"))]
    async fn get_cell_triggers(&self, cell_id: &CellId) -> ConductorApiResult<QueueTriggers> {
        let cell = self.cell_by_id(cell_id).await?;
        Ok(cell.triggers().clone())
    }

    #[cfg(any(test, feature = "test_utils"))]
    async fn get_state_from_handle(&self) -> ConductorApiResult<ConductorState> {
        let lock = self.conductor.read().await;
        Ok(lock.get_state_from_handle().await?)
    }

    #[cfg(any(test, feature = "test_utils"))]
    async fn add_test_app_interface(
        &self,
        id: super::state::AppInterfaceId,
    ) -> ConductorResult<()> {
        let mut lock = self.conductor.write().await;
        lock.add_test_app_interface(id).await
    }

    #[cfg(any(test, feature = "test_utils"))]
    fn should_skip_publish(&self) -> bool {
        self.skip_publish.load(std::sync::atomic::Ordering::Relaxed)
    }

    #[cfg(any(test, feature = "test_utils"))]
    fn set_skip_publish(&self, skip_publish: bool) {
        self.skip_publish
            .store(skip_publish, std::sync::atomic::Ordering::Relaxed);
    }
}

impl<DS: DnaStore + 'static> ConductorHandleImpl<DS> {
    async fn cell_by_id(&self, cell_id: &CellId) -> ConductorApiResult<Arc<Cell>> {
        let lock = self.conductor.read().await;
        Ok(lock.cell_by_id(cell_id)?)
    }

    /// Add cells to the map then join the network then initialize workflows.
    async fn initialize_cells(&self, cells: Vec<(Cell, InitialQueueTriggers)>) {
        let (cells, triggers): (Vec<_>, Vec<_>) = cells.into_iter().unzip();
        let networks: Vec<_> = cells
            .iter()
            .map(|cell| cell.holochain_p2p_cell().clone())
            .collect();
        // Add the cells to the conductor map.
        // This write lock can't be held while join is awaited as join calls the conductor.
        // Cells need to be in the map before join is called so it can route the call.
        self.conductor.write().await.add_cells(cells);
        // Join the network but ignore errors because the
        // space retries joining all cells every 5 minutes.
        futures::stream::iter(networks)
            .for_each_concurrent(100, |mut network| async move {
                match tokio::time::timeout(JOIN_NETWORK_TIMEOUT, network.join()).await {
                    Ok(Err(e)) => {
                        tracing::info!(failed_to_join_network = ?e);
                    }
                    Err(_) => {
                        tracing::info!("Timed out trying to join the network");
                    }
                    Ok(Ok(_)) => (),
                }
            })
            .await;

        // Now we can trigger the workflows.
        for trigger in triggers {
            trigger.initialize_workflows();
        }
    }
}
