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
//!    .test(&envs, &[])
//!    .await
//!    .unwrap();
//!
//! // handles are cloneable
//! let handle2 = handle.clone();
//!
//! assert_eq!(handle.list_dnas(), vec![]);
//! handle.shutdown();
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
use super::conductor::CellStatus;
use super::config::AdminInterfaceConfig;
use super::error::ConductorResult;
use super::integration_dump;
use super::interface::SignalBroadcaster;
use super::manager::spawn_task_manager;
use super::manager::TaskManagerClient;
use super::manager::TaskManagerRunHandle;
use super::p2p_agent_store;
use super::p2p_agent_store::all_agent_infos;
use super::p2p_agent_store::get_agent_info_signed;
use super::p2p_agent_store::inject_agent_infos;
use super::p2p_agent_store::list_all_agent_info;
use super::p2p_agent_store::list_all_agent_info_signed_near_basis;
use super::Cell;
use super::CellError;
use super::Conductor;
use crate::conductor::p2p_agent_store::get_single_agent_info;
use crate::conductor::p2p_agent_store::query_peer_density;
use crate::conductor::p2p_agent_store::P2pBatch;
use crate::conductor::p2p_metrics::put_metric_datum;
use crate::conductor::p2p_metrics::query_metrics;
use crate::core::ribosome::real_ribosome::RealRibosome;
use crate::core::workflow::ZomeCallResult;
use derive_more::From;
use futures::future::FutureExt;
use futures::StreamExt;
use holochain_conductor_api::conductor::EnvironmentRootPath;
use holochain_conductor_api::AppStatusFilter;
use holochain_conductor_api::InstalledAppInfo;
use holochain_conductor_api::JsonDump;
use holochain_keystore::MetaLairClient;
use holochain_p2p::event::HolochainP2pEvent;
use holochain_p2p::event::HolochainP2pEvent::*;
use holochain_p2p::DnaHashExt;
use holochain_p2p::HolochainP2pCell;
use holochain_p2p::HolochainP2pCellT;
use holochain_sqlite::db::DbKind;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_state::source_chain;
use holochain_types::prelude::*;
use kitsune_p2p::agent_store::AgentInfoSigned;
use kitsune_p2p::KitsuneSpace;
use kitsune_p2p_types::config::JOIN_NETWORK_TIMEOUT;
use std::collections::HashMap;
use std::{collections::HashSet, sync::Arc};
use tracing::*;

#[cfg(any(test, feature = "test_utils"))]
use super::state::ConductorState;
#[cfg(any(test, feature = "test_utils"))]
use crate::core::queue_consumer::QueueTriggers;

/// A handle to the Conductor that can easily be passed around and cheaply cloned
pub type ConductorHandle = Arc<dyn ConductorHandleT>;

/// A list of Cells which failed to start, and why
pub type CellStartupErrors = Vec<(CellId, CellError)>;

/// Base trait for ConductorHandle
#[mockall::automock]
#[async_trait::async_trait]
pub trait ConductorHandleT: Send + Sync {
    /// Returns error if conductor is shutting down
    fn check_running(&self) -> ConductorResult<()>;

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
    ) -> ConductorResult<CellStartupErrors>;

    /// Add a collection of admin interfaces from config
    async fn add_admin_interfaces(
        self: Arc<Self>,
        configs: Vec<AdminInterfaceConfig>,
    ) -> ConductorResult<()>;

    /// Add an app interface
    async fn add_app_interface(self: Arc<Self>, port: u16) -> ConductorResult<u16>;

    /// List the app interfaces currently install.
    async fn list_app_interfaces(&self) -> ConductorResult<Vec<u16>>;

    /// Install a [DnaFile] in this Conductor
    async fn register_dna(&self, dna: DnaFile) -> ConductorResult<()>;

    /// Get the list of hashes of installed Dnas in this Conductor
    fn list_dnas(&self) -> Vec<DnaHash>;

    /// Get a [Dna] from the [DnaStore]
    fn get_dna(&self, hash: &DnaHash) -> Option<DnaFile>;

    /// Get an instance of a [RealRibosome] for the DnaHash
    fn get_ribosome(&self, dna_hash: &DnaHash) -> ConductorResult<RealRibosome>;

    /// Get a [EntryDef] from the [EntryDefBuffer]
    fn get_entry_def(&self, key: &EntryDefBufferKey) -> Option<EntryDef>;

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
    fn get_arbitrary_admin_websocket_port(&self) -> Option<u16>;

    /// Return the JoinHandle for all managed tasks, which when resolved will
    /// signal that the Conductor has completely shut down.
    ///
    /// NB: The JoinHandle is not cloneable,
    /// so this can only ever be called successfully once.
    fn take_shutdown_handle(&self) -> Option<TaskManagerRunHandle>;

    /// Send a signal to all managed tasks asking them to end ASAP.
    fn shutdown(&self);

    /// Request access to this conductor's keystore
    fn keystore(&self) -> &MetaLairClient;

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
    ) -> ConductorResult<StoppedApp>;

    /// Uninstall an app from the state DB and remove all running Cells
    async fn uninstall_app(self: Arc<Self>, app: &InstalledAppId) -> ConductorResult<()>;

    /// Adjust app statuses (via state transitions) to match the current
    /// reality of which Cells are present in the conductor.
    async fn reconcile_app_status_with_cell_status(
        &self,
        app_ids: Option<HashSet<InstalledAppId>>,
    ) -> ConductorResult<AppStatusFx>;

    /// Adjust which cells are present in the Conductor (adding and removing as
    /// needed) to match the current reality of all app statuses.
    /// - If a Cell is used by at least one Running app, then ensure it is added
    /// - If a Cell is used by no running apps, then ensure it is removed.
    async fn reconcile_cell_status_with_app_status(
        self: Arc<Self>,
    ) -> ConductorResult<CellStartupErrors>;

    /// Activate an app
    async fn enable_app(
        self: Arc<Self>,
        app_id: InstalledAppId,
    ) -> ConductorResult<(InstalledApp, CellStartupErrors)>;

    /// Disable an app
    async fn disable_app(
        self: Arc<Self>,
        app_id: InstalledAppId,
        reason: DisabledAppReason,
    ) -> ConductorResult<InstalledApp>;

    /// Start an enabled but stopped (paused) app
    async fn start_app(self: Arc<Self>, app_id: InstalledAppId) -> ConductorResult<InstalledApp>;

    /// Start the scheduler. All ephemeral tasks are deleted.
    async fn start_scheduler(self: Arc<Self>, interval_period: std::time::Duration);

    /// Dispatch all due scheduled functions.
    async fn dispatch_scheduled_fns(self: Arc<Self>);

    /// Stop a running app while leaving it enabled. FOR TESTING ONLY.
    #[cfg(any(test, feature = "test_utils"))]
    async fn pause_app(
        self: Arc<Self>,
        app_id: InstalledAppId,
        reason: PausedAppReason,
    ) -> ConductorResult<InstalledApp>;

    /// List Cell Ids
    fn list_cell_ids(&self, filter: Option<CellStatus>) -> Vec<CellId>;

    /// List Active AppIds
    async fn list_running_apps(&self) -> ConductorResult<Vec<InstalledAppId>>;

    /// List Apps with their information
    async fn list_apps(
        &self,
        status_filter: Option<AppStatusFilter>,
    ) -> ConductorResult<Vec<InstalledAppInfo>>;

    /// Get the IDs of all active installed Apps which use this Cell
    async fn list_running_apps_for_required_cell_id(
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
    fn print_setup(&self);

    /// Retrieve the environment for this cell.
    fn get_cell_env_readonly(&self, cell_id: &CellId) -> ConductorApiResult<EnvRead>;

    /// Manually remove some cells. Should only be used when handling errors in Cells,
    /// allowing individual Cells to be shut down.
    async fn remove_cells(&self, cell_ids: &[CellId]);

    /// Retrieve the environment for this cell. FOR TESTING ONLY.
    #[cfg(any(test, feature = "test_utils"))]
    fn get_cell_env(&self, cell_id: &CellId) -> ConductorApiResult<EnvWrite>;

    /// Retrieve the database for this cell. FOR TESTING ONLY.
    #[cfg(any(test, feature = "test_utils"))]
    fn get_cache_env(&self, cell_id: &CellId) -> ConductorApiResult<EnvWrite>;

    /// Retrieve the database for networking. FOR TESTING ONLY.
    #[cfg(any(test, feature = "test_utils"))]
    fn get_p2p_env(&self, space: Arc<KitsuneSpace>) -> EnvWrite;

    /// Retrieve Senders for triggering workflows. FOR TESTING ONLY.
    #[cfg(any(test, feature = "test_utils"))]
    fn get_cell_triggers(&self, cell_id: &CellId) -> ConductorApiResult<QueueTriggers>;

    /// Retrieve the ConductorState. FOR TESTING ONLY.
    #[cfg(any(test, feature = "test_utils"))]
    async fn get_state_from_handle(&self) -> ConductorApiResult<ConductorState>;

    /// Add a "test" app interface for sending and receiving signals. FOR TESTING ONLY.
    #[cfg(any(test, feature = "test_utils"))]
    async fn add_test_app_interface(&self, id: super::state::AppInterfaceId)
        -> ConductorResult<()>;

    #[cfg(any(test, feature = "test_utils"))]
    /// Check whether this conductor should skip publish.
    fn should_skip_publish(&self) -> bool;

    #[cfg(any(test, feature = "test_utils"))]
    /// For testing we can choose to skip publish.
    fn set_skip_publish(&self, skip_publish: bool);

    /// Manually coerce cells to a given CellStatus. FOR TESTING ONLY.
    #[cfg(any(test, feature = "test_utils"))]
    fn update_cell_status(&self, cell_ids: &[CellId], status: CellStatus);

    /// Manually coerce app to a given AppStatus. FOR TESTING ONLY.
    #[cfg(any(test, feature = "test_utils"))]
    async fn transition_app_status(
        &self,
        app_id: InstalledAppId,
        transition: AppStatusTransition,
    ) -> ConductorResult<(InstalledApp, AppStatusFx)>;

    // TODO: would be nice to have methods for accessing the underlying Conductor,
    // but this trait doesn't know the concrete type of Conductor underlying,
    // and using generics seems problematic with mockall::automock.
    // Something like this would be desirable, but ultimately doesn't work.
    //
    // type DS: Send + Sync + DnaStore;
    //
    // /// Get immutable access to the inner conductor state via a read lock
    // async fn conductor<F, T>(&self, f: F) -> ConductorApiResult<T>
    // where
    //     F: FnOnce(&Conductor<<Self as ConductorHandleT>::DS>) -> ConductorApiResult<T>,
    // {
    //     let c = self.conductor.read().await;
    //     f(&c)
    // }
    //
    // /// Get mutable access to the inner conductor state via a write lock
    // async fn conductor_mut<F, T>(&self, f: F) -> ConductorApiResult<T>
    // where
    //     F: FnOnce(&mut Conductor<<Self as ConductorHandleT>::DS>) -> ConductorApiResult<T>,
    // {
    //     let mut c = self.conductor.write().await;
    //     f(&mut c)
    // }
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
    pub(super) conductor: Conductor<DS>,
    pub(super) keystore: MetaLairClient,
    pub(super) holochain_p2p: holochain_p2p::HolochainP2pRef,

    /// The root environment directory where all environments are created
    pub(super) root_env_dir: EnvironmentRootPath,

    /// The database for storing AgentInfoSigned
    pub(super) p2p_env: Arc<parking_lot::Mutex<HashMap<Arc<KitsuneSpace>, EnvWrite>>>,

    /// The database for storing p2p MetricDatum(s)
    pub(super) p2p_metrics_env: Arc<parking_lot::Mutex<HashMap<Arc<KitsuneSpace>, EnvWrite>>>,

    /// Database sync level
    pub(super) db_sync_level: DbSyncLevel,

    /// The batch sender for writes to the p2p database.
    pub(super) p2p_batch_senders:
        Arc<parking_lot::Mutex<HashMap<Arc<KitsuneSpace>, tokio::sync::mpsc::Sender<P2pBatch>>>>,

    // Testing:
    #[cfg(any(test, feature = "test_utils"))]
    /// All conductors should skip publishing.
    /// This is useful for testing gossip.
    pub skip_publish: std::sync::atomic::AtomicBool,
}

#[async_trait::async_trait]
impl<DS: DnaStore + 'static> ConductorHandleT for ConductorHandleImpl<DS> {
    /// Check that shutdown has not been called
    fn check_running(&self) -> ConductorResult<()> {
        self.conductor.check_running()
    }

    async fn add_admin_interfaces(
        self: Arc<Self>,
        configs: Vec<AdminInterfaceConfig>,
    ) -> ConductorResult<()> {
        self.conductor
            .add_admin_interfaces_via_handle(configs, self.clone())
            .await
    }

    async fn initialize_conductor(
        self: Arc<Self>,
        admin_configs: Vec<AdminInterfaceConfig>,
    ) -> ConductorResult<CellStartupErrors> {
        self.load_dnas().await?;

        // Start the task manager
        let (task_add_sender, run_handle) = spawn_task_manager(self.clone());
        let (task_stop_broadcaster, _) = tokio::sync::broadcast::channel::<()>(1);
        self.conductor.task_manager.share_mut(|tm| {
            if tm.is_some() {
                panic!("Cannot start task manager twice");
            }
            *tm = Some(TaskManagerClient::new(
                task_add_sender,
                task_stop_broadcaster,
                run_handle,
            ));
        });

        self.conductor
            .add_admin_interfaces_via_handle(admin_configs, self.clone())
            .await?;

        self.conductor
            .startup_app_interfaces_via_handle(self.clone())
            .await?;

        // We don't care what fx are returned here, since all cells need to
        // be spun up
        let _ = self.conductor.start_paused_apps().await?;

        self.process_app_status_fx(AppStatusFx::SpinUp, None).await
    }

    async fn add_app_interface(self: Arc<Self>, port: u16) -> ConductorResult<u16> {
        self.conductor
            .add_app_interface_via_handle(either::Left(port), self.clone())
            .await
    }

    async fn list_app_interfaces(&self) -> ConductorResult<Vec<u16>> {
        self.conductor.list_app_interfaces().await
    }

    async fn register_dna(&self, dna: DnaFile) -> ConductorResult<()> {
        self.register_genotype(dna.clone()).await?;
        self.conductor.register_phenotype(dna);
        Ok(())
    }

    async fn load_dnas(&self) -> ConductorResult<()> {
        let (dnas, entry_defs) = self.conductor.load_wasms_into_dna_files().await?;
        self.conductor.dna_store().share_mut(|ds| {
            ds.add_dnas(dnas);
            ds.add_entry_defs(entry_defs);
        });
        Ok(())
    }

    fn list_dnas(&self) -> Vec<DnaHash> {
        self.conductor.dna_store().share_ref(|ds| ds.list())
    }

    fn get_dna(&self, hash: &DnaHash) -> Option<DnaFile> {
        self.conductor.dna_store().share_ref(|ds| ds.get(hash))
    }

    fn get_ribosome(&self, dna_hash: &DnaHash) -> ConductorResult<RealRibosome> {
        self.conductor.get_ribosome(dna_hash)
    }

    fn get_entry_def(&self, key: &EntryDefBufferKey) -> Option<EntryDef> {
        self.conductor
            .dna_store()
            .share_ref(|ds| ds.get_entry_def(key))
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
                peer_data, respond, ..
            } => {
                let sender = self.p2p_batch_sender(space);
                let (result_sender, response) = tokio::sync::oneshot::channel();
                let _ = sender
                    .send(P2pBatch {
                        peer_data,
                        result_sender,
                    })
                    .await;
                let res = match response.await {
                    Ok(r) => r.map_err(holochain_p2p::HolochainP2pError::other),
                    Err(e) => Err(holochain_p2p::HolochainP2pError::other(e)),
                };
                respond.respond(Ok(async move { res }.boxed().into()));
            }
            GetAgentInfoSigned {
                kitsune_space,
                kitsune_agent,
                respond,
                ..
            } => {
                let env = { self.p2p_env(space) };
                let res = get_agent_info_signed(env, kitsune_space, kitsune_agent)
                    .map_err(holochain_p2p::HolochainP2pError::other);
                respond.respond(Ok(async move { res }.boxed().into()));
            }
            QueryAgentInfoSigned {
                kitsune_space,
                agents,
                respond,
                ..
            } => {
                let env = { self.p2p_env(space) };
                let res = list_all_agent_info(env, kitsune_space)
                    .map(|infos| match agents {
                        Some(agents) => infos
                            .into_iter()
                            .filter(|info| agents.contains(&info.agent))
                            .collect(),
                        None => infos,
                    })
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
                let env = { self.p2p_env(space) };
                let res = env
                    .conn()?
                    .p2p_gossip_query_agents(since_ms, until_ms, (*arc_set).clone())
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
                let env = { self.p2p_env(space) };
                let res =
                    list_all_agent_info_signed_near_basis(env, kitsune_space, basis_loc, limit)
                        .map_err(holochain_p2p::HolochainP2pError::other);
                respond.respond(Ok(async move { res }.boxed().into()));
            }
            QueryPeerDensity {
                kitsune_space,
                dht_arc,
                respond,
                ..
            } => {
                let env = { self.p2p_env(space) };
                let res = query_peer_density(env, kitsune_space, dht_arc)
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
                let env = { self.p2p_metrics_env(space) };
                let res = put_metric_datum(env, agent, metric, timestamp)
                    .await
                    .map_err(holochain_p2p::HolochainP2pError::other);
                respond.respond(Ok(async move { res }.boxed().into()));
            }
            QueryMetrics { respond, query, .. } => {
                let env = { self.p2p_metrics_env(space) };
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
                let signature = to_agent.sign_raw(self.keystore(), data.into()).await?;
                respond.respond(Ok(async move { Ok(signature) }.boxed().into()));
            }
            HolochainP2pEvent::CallRemote { .. }
            | CountersigningAuthorityResponse { .. }
            | Publish { .. }
            | GetValidationPackage { .. }
            | Get { .. }
            | GetMeta { .. }
            | GetLinks { .. }
            | GetAgentActivity { .. }
            | ValidationReceiptReceived { .. }
            | FetchOpData { .. } => {
                let cell_id = CellId::new(event.dna_hash().clone(), event.target_agents().clone());
                let cell = self.cell_by_id(&cell_id)?;
                cell.handle_holochain_p2p_event(event).await?;
            }

            // This event does not have a single Cell as a target, so we handle
            // it at the conductor level.
            // TODO: perhaps we can do away with the assumption that each event
            //       is meant for a single Cell, i.e. allow batching in general
            HolochainP2pEvent::QueryOpHashes {
                dna_hash,
                to_agents,
                window,
                max_ops,
                include_limbo,
                respond,
                ..
            } => {
                let mut hashes_and_times = Vec::with_capacity(to_agents.len());

                // For each cell collect the hashes and times that fit within the
                // agents interval and time window.
                for (agent, arc_set) in to_agents {
                    let cell_id = CellId::new(dna_hash.clone(), agent);
                    let cell = self.cell_by_id(&cell_id)?;
                    match cell
                        .handle_query_op_hashes(arc_set, window.clone(), include_limbo)
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
                    .get(max_ops)
                    .or_else(|| hashes_and_times.last())
                    .map(|(_, t)| *t);

                // Extract the hashes.
                let hashes: Vec<_> = hashes_and_times
                    .into_iter()
                    .take(max_ops)
                    .map(|(h, _)| h)
                    .collect();

                // The range is exclusive so we add one to the end.
                let range = start.and_then(|s| {
                    end.map(|e| {
                        (
                            hashes,
                            s..(e.saturating_add(&std::time::Duration::from_millis(1))),
                        )
                    })
                });

                respond.respond(Ok(async move { Ok(range) }.boxed().into()));
            }
        }
        Ok(())
    }

    async fn call_zome(&self, call: ZomeCall) -> ConductorApiResult<ZomeCallResult> {
        let cell = self.cell_by_id(&call.cell_id)?;
        Ok(cell.call_zome(call, None).await?)
    }

    async fn call_zome_with_workspace(
        &self,
        call: ZomeCall,
        workspace_lock: HostFnWorkspace,
    ) -> ConductorApiResult<ZomeCallResult> {
        debug!(cell_id = ?call.cell_id);
        let cell = self.cell_by_id(&call.cell_id)?;
        Ok(cell.call_zome(call, Some(workspace_lock)).await?)
    }

    fn take_shutdown_handle(&self) -> Option<TaskManagerRunHandle> {
        self.conductor.take_shutdown_handle()
    }

    fn get_arbitrary_admin_websocket_port(&self) -> Option<u16> {
        self.conductor.get_arbitrary_admin_websocket_port()
    }

    fn shutdown(&self) {
        self.conductor.shutdown()
    }

    fn keystore(&self) -> &MetaLairClient {
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
            role_id,
            membrane_proof,
        } = payload;
        let cell_id = CellId::new(dna_hash, agent_key);
        let cells = vec![(cell_id.clone(), membrane_proof)];

        // Gather the directory and keystore to avoid holding the conductor read lock.
        let root_env_dir = std::path::PathBuf::from(self.conductor.root_env_dir().clone());
        let keystore = self.conductor.keystore().clone();
        // Run genesis on cells.
        crate::conductor::conductor::genesis_cells(
            root_env_dir,
            keystore,
            cells,
            self.clone(),
            self.db_sync_level,
        )
        .await?;

        let properties = properties.unwrap_or_else(|| ().into());
        let cell_id = self
            .conductor
            .add_clone_cell_to_app(installed_app_id, role_id, properties)
            .await?;
        Ok(cell_id)
    }

    async fn destroy_clone_cell(self: Arc<Self>, _cell_id: CellId) -> ConductorResult<()> {
        todo!()
    }

    async fn install_app(
        self: Arc<Self>,
        installed_app_id: InstalledAppId,
        cell_data: Vec<(InstalledCell, Option<MembraneProof>)>,
    ) -> ConductorResult<()> {
        // Gather the directory and keystore to avoid holding the conductor read lock.
        let root_env_dir = std::path::PathBuf::from(self.conductor.root_env_dir().clone());
        let keystore = self.conductor.keystore().clone();

        crate::conductor::conductor::genesis_cells(
            root_env_dir,
            keystore,
            cell_data
                .iter()
                .map(|(c, p)| (c.as_id().clone(), p.clone()))
                .collect(),
            self.clone(),
            self.db_sync_level,
        )
        .await?;

        let cell_data = cell_data.into_iter().map(|(c, _)| c);
        let app = InstalledAppCommon::new_legacy(installed_app_id, cell_data)?;

        // Update the db
        let _ = self.conductor.add_disabled_app_to_db(app).await?;

        Ok(())
    }

    async fn install_app_bundle(
        self: Arc<Self>,
        payload: InstallAppBundlePayload,
    ) -> ConductorResult<StoppedApp> {
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

        // Gather the directory and keystore to avoid holding the conductor read lock.
        let root_env_dir = std::path::PathBuf::from(self.conductor.root_env_dir().clone());
        let keystore = self.conductor.keystore().clone();

        crate::conductor::conductor::genesis_cells(
            root_env_dir,
            keystore,
            cells_to_create,
            self.clone(),
            self.db_sync_level,
        )
        .await?;

        let roles = ops.roles;
        let app = InstalledAppCommon::new(installed_app_id, agent_key, roles);

        // Update the db
        let stopped_app = self.conductor.add_disabled_app_to_db(app).await?;

        Ok(stopped_app)
    }

    /// Start the scheduler. None is not an option.
    /// Calling this will:
    /// - Delete/unschedule all ephemeral scheduled functions GLOBALLY
    /// - Add an interval that runs IN ADDITION to previous invocations
    /// So ideally this would be called ONCE per conductor lifecyle ONLY.
    async fn start_scheduler(self: Arc<Self>, interval_period: std::time::Duration) {
        // Clear all ephemeral cruft in all cells before starting a scheduler.
        let cell_arcs = {
            let mut cell_arcs = vec![];
            for cell_id in self.conductor.running_cell_ids() {
                if let Ok(cell_arc) = self.cell_by_id(&cell_id) {
                    cell_arcs.push(cell_arc);
                }
            }
            cell_arcs
        };
        let tasks = cell_arcs
            .into_iter()
            .map(|cell_arc| cell_arc.delete_all_ephemeral_scheduled_fns());
        futures::future::join_all(tasks).await;

        let scheduler_handle = self.clone();
        tokio::task::spawn(async move {
            let mut interval = tokio::time::interval(interval_period);
            loop {
                interval.tick().await;
                scheduler_handle.clone().dispatch_scheduled_fns().await;
            }
        });
    }

    /// The scheduler wants to dispatch any functions that are due.
    async fn dispatch_scheduled_fns(self: Arc<Self>) {
        let cell_arcs = {
            let mut cell_arcs = vec![];
            for cell_id in self.conductor.running_cell_ids() {
                if let Ok(cell_arc) = self.cell_by_id(&cell_id) {
                    cell_arcs.push(cell_arc);
                }
            }
            cell_arcs
        };

        let tasks = cell_arcs
            .into_iter()
            .map(|cell_arc| cell_arc.dispatch_scheduled_fns());
        futures::future::join_all(tasks).await;
    }

    #[tracing::instrument(skip(self))]
    async fn reconcile_app_status_with_cell_status(
        &self,
        app_ids: Option<HashSet<InstalledAppId>>,
    ) -> ConductorResult<AppStatusFx> {
        self.conductor
            .reconcile_app_status_with_cell_status(app_ids)
            .await
    }

    #[tracing::instrument(skip(self))]
    async fn reconcile_cell_status_with_app_status(
        self: Arc<Self>,
    ) -> ConductorResult<CellStartupErrors> {
        self.conductor.remove_dangling_cells().await?;

        let results = self
            .create_and_add_initialized_cells_for_running_apps(self.clone())
            .await?;
        Ok(results)
    }

    #[tracing::instrument(skip(self))]
    async fn enable_app(
        self: Arc<Self>,
        app_id: InstalledAppId,
    ) -> ConductorResult<(InstalledApp, CellStartupErrors)> {
        let (app, delta) = self
            .conductor
            .transition_app_status(app_id.clone(), AppStatusTransition::Enable)
            .await?;
        let errors = self
            .process_app_status_fx(delta, Some(vec![app_id.to_owned()].into_iter().collect()))
            .await?;
        Ok((app, errors))
    }

    #[tracing::instrument(skip(self))]
    async fn disable_app(
        self: Arc<Self>,
        app_id: InstalledAppId,
        reason: DisabledAppReason,
    ) -> ConductorResult<InstalledApp> {
        let (app, delta) = self
            .conductor
            .transition_app_status(app_id.clone(), AppStatusTransition::Disable(reason))
            .await?;
        self.process_app_status_fx(delta, Some(vec![app_id.to_owned()].into_iter().collect()))
            .await?;
        Ok(app)
    }

    #[tracing::instrument(skip(self))]
    async fn start_app(self: Arc<Self>, app_id: InstalledAppId) -> ConductorResult<InstalledApp> {
        let (app, delta) = self
            .conductor
            .transition_app_status(app_id.clone(), AppStatusTransition::Start)
            .await?;
        self.process_app_status_fx(delta, Some(vec![app_id.to_owned()].into_iter().collect()))
            .await?;
        Ok(app)
    }

    #[tracing::instrument(skip(self))]
    #[cfg(any(test, feature = "test_utils"))]
    async fn pause_app(
        self: Arc<Self>,
        app_id: InstalledAppId,
        reason: PausedAppReason,
    ) -> ConductorResult<InstalledApp> {
        let (app, delta) = self
            .conductor
            .transition_app_status(app_id.clone(), AppStatusTransition::Pause(reason))
            .await?;
        self.process_app_status_fx(delta, Some(vec![app_id.clone()].into_iter().collect()))
            .await?;
        Ok(app)
    }

    #[tracing::instrument(skip(self))]
    async fn uninstall_app(
        self: Arc<Self>,
        installed_app_id: &InstalledAppId,
    ) -> ConductorResult<()> {
        let self_clone = self.clone();
        let app = self.conductor.remove_app_from_db(installed_app_id).await?;
        tracing::debug!(msg = "Removed app from db.", app = ?app);

        // Remove cells which may now be dangling due to the removed app
        self_clone
            .process_app_status_fx(AppStatusFx::SpinDown, None)
            .await?;
        Ok(())
    }

    fn list_cell_ids(&self, filter: Option<CellStatus>) -> Vec<CellId> {
        self.conductor.list_cell_ids(filter)
    }

    async fn list_running_apps(&self) -> ConductorResult<Vec<InstalledAppId>> {
        self.conductor.list_running_apps().await
    }

    async fn list_apps(
        &self,
        status_filter: Option<AppStatusFilter>,
    ) -> ConductorResult<Vec<InstalledAppInfo>> {
        self.conductor.list_apps(status_filter).await
    }

    async fn list_running_apps_for_required_cell_id(
        &self,
        cell_id: &CellId,
    ) -> ConductorResult<HashSet<InstalledAppId>> {
        self.conductor.list_running_apps_for_cell_id(cell_id).await
    }

    async fn dump_cell_state(&self, cell_id: &CellId) -> ConductorApiResult<String> {
        let cell = self.conductor.cell_by_id(cell_id)?;
        let arc = cell.env();
        let space = cell_id.dna_hash().to_kitsune();
        let p2p_env = self
            .p2p_env
            .lock()
            .get(&space)
            .cloned()
            .expect("invalid cell space");

        let peer_dump = p2p_agent_store::dump_state(p2p_env.into(), Some(cell_id.clone()))?;
        let source_chain_dump =
            source_chain::dump_state(arc.clone().into(), cell_id.agent_pubkey().clone()).await?;

        let out = JsonDump {
            peer_dump,
            source_chain_dump,
            integration_dump: integration_dump(&arc.clone().into()).await?,
        };
        // Add summary
        let summary = out.to_string();
        let out = (out, summary);
        Ok(serde_json::to_string_pretty(&out)?)
    }

    async fn signal_broadcaster(&self) -> SignalBroadcaster {
        self.conductor.signal_broadcaster()
    }

    async fn get_app_info(
        &self,
        installed_app_id: &InstalledAppId,
    ) -> ConductorResult<Option<InstalledAppInfo>> {
        Ok(self
            .conductor
            .get_state()
            .await?
            .get_app_info(installed_app_id))
    }

    async fn add_agent_infos(&self, agent_infos: Vec<AgentInfoSigned>) -> ConductorApiResult<()> {
        let mut space_map = HashMap::new();
        for agent_info_signed in agent_infos {
            let space = agent_info_signed.space.clone();
            space_map
                .entry(space)
                .or_insert_with(Vec::new)
                .push(agent_info_signed);
        }
        for (space, agent_infos) in space_map {
            let env = self.p2p_env(space);
            inject_agent_infos(env, agent_infos.iter()).await?;
        }
        Ok(())
    }

    async fn get_agent_infos(
        &self,
        cell_id: Option<CellId>,
    ) -> ConductorApiResult<Vec<AgentInfoSigned>> {
        match cell_id {
            Some(c) => {
                let (d, a) = c.into_dna_and_agent();
                let space = d.to_kitsune();
                let env = self.p2p_env(space);
                Ok(get_single_agent_info(env.into(), d, a)?
                    .map(|a| vec![a])
                    .unwrap_or_default())
            }
            None => {
                let mut out = Vec::new();
                // collecting so the mutex lock can close
                let envs = self.p2p_env.lock().values().cloned().collect::<Vec<_>>();
                for env in envs {
                    out.append(&mut all_agent_infos(env.into())?);
                }
                Ok(out)
            }
        }
    }

    fn print_setup(&self) {
        self.conductor.print_setup()
    }

    fn get_cell_env_readonly(&self, cell_id: &CellId) -> ConductorApiResult<EnvRead> {
        let cell = self.cell_by_id(cell_id)?;
        Ok(cell.env().clone().into())
    }

    async fn remove_cells(&self, cell_ids: &[CellId]) {
        self.conductor.remove_cells(cell_ids.to_vec()).await
    }

    #[cfg(any(test, feature = "test_utils"))]
    fn get_cell_env(&self, cell_id: &CellId) -> ConductorApiResult<EnvWrite> {
        let cell = self.cell_by_id(cell_id)?;
        Ok(cell.env().clone())
    }

    #[cfg(any(test, feature = "test_utils"))]
    fn get_cache_env(&self, cell_id: &CellId) -> ConductorApiResult<EnvWrite> {
        let cell = self.cell_by_id(cell_id)?;
        Ok(cell.cache().clone())
    }

    #[cfg(any(test, feature = "test_utils"))]
    fn get_p2p_env(&self, space: Arc<KitsuneSpace>) -> EnvWrite {
        self.p2p_env(space)
    }

    #[cfg(any(test, feature = "test_utils"))]
    fn get_cell_triggers(&self, cell_id: &CellId) -> ConductorApiResult<QueueTriggers> {
        let cell = self.cell_by_id(cell_id)?;
        Ok(cell.triggers().clone())
    }

    #[cfg(any(test, feature = "test_utils"))]
    async fn get_state_from_handle(&self) -> ConductorApiResult<ConductorState> {
        Ok(self.conductor.get_state_from_handle().await?)
    }

    #[cfg(any(test, feature = "test_utils"))]
    async fn add_test_app_interface(
        &self,
        id: super::state::AppInterfaceId,
    ) -> ConductorResult<()> {
        self.conductor.add_test_app_interface(id).await
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

    #[cfg(any(test, feature = "test_utils"))]
    fn update_cell_status(&self, cell_ids: &[CellId], status: CellStatus) {
        self.conductor.update_cell_status(cell_ids, status)
    }

    #[cfg(any(test, feature = "test_utils"))]
    async fn transition_app_status(
        &self,
        app_id: InstalledAppId,
        transition: AppStatusTransition,
    ) -> ConductorResult<(InstalledApp, AppStatusFx)> {
        self.conductor
            .transition_app_status(app_id, transition)
            .await
    }
}

impl<DS: DnaStore + 'static> ConductorHandleImpl<DS> {
    fn cell_by_id(&self, cell_id: &CellId) -> ConductorApiResult<Arc<Cell>> {
        Ok(self.conductor.cell_by_id(cell_id)?)
    }

    /// Install just the "code parts" (the wasm and entry defs) of a dna
    async fn register_genotype(&self, dna: DnaFile) -> ConductorResult<()> {
        let entry_defs = self.conductor.register_dna_wasm(dna).await?;
        self.conductor.register_dna_entry_defs(entry_defs);
        Ok(())
    }

    /// Deal with the side effects of an app status state transition
    async fn process_app_status_fx(
        self: Arc<Self>,
        delta: AppStatusFx,
        app_ids: Option<HashSet<InstalledAppId>>,
    ) -> ConductorResult<CellStartupErrors> {
        use AppStatusFx::*;
        let mut last = (delta, vec![]);
        loop {
            tracing::debug!(msg = "Processing app status delta", delta = ?last.0);
            last = match last.0 {
                NoChange => break,
                SpinDown => {
                    // Reconcile cell status so that dangling cells can leave the network and be removed
                    let errors = self.clone().reconcile_cell_status_with_app_status().await?;

                    // TODO: This should probably be emitted over the admin interface
                    if !errors.is_empty() {
                        error!(msg = "Errors when trying to stop app(s)", ?errors);
                    }

                    (NoChange, errors)
                }
                SpinUp | Both => {
                    // Reconcile cell status so that missing/pending cells can become fully joined
                    let errors = self.clone().reconcile_cell_status_with_app_status().await?;

                    // Reconcile app status in case some cells failed to join, so the app can be paused
                    let delta = self
                        .clone()
                        .reconcile_app_status_with_cell_status(app_ids.clone())
                        .await?;

                    // TODO: This should probably be emitted over the admin interface
                    if !errors.is_empty() {
                        error!(msg = "Errors when trying to start app(s)", ?errors);
                    }

                    (delta, errors)
                }
            };
        }

        Ok(last.1)
    }

    /// Create any Cells which are missing for any running apps, then initialize
    /// and join them. (Joining could take a while.)
    pub(super) async fn create_and_add_initialized_cells_for_running_apps(
        &self,
        conductor_handle: ConductorHandle,
    ) -> ConductorResult<CellStartupErrors> {
        let results = self
            .conductor
            .create_cells_for_running_apps(conductor_handle)
            .await?;
        let (new_cells, errors): (Vec<_>, Vec<_>) = results.into_iter().partition(Result::is_ok);

        let new_cells = new_cells
            .into_iter()
            // We can unwrap the successes because of the partition
            .map(Result::unwrap)
            .collect();

        let errors = errors
            .into_iter()
            // throw away the non-Debug types which will be unwrapped away anyway
            .map(|r| r.map(|_| ()))
            // We can unwrap the errors because of the partition
            .map(Result::unwrap_err)
            .collect();

        // Add the newly created cells to the Conductor with the PendingJoin
        // status, and start their workflow loops
        self.conductor.add_and_initialize_cells(new_cells);

        // Join these newly created cells to the network
        // (as well as any others which need joining)
        self.join_all_pending_cells().await;

        Ok(errors)
    }

    /// Attempt to join all PendingJoin cells to the kitsune network.
    /// Returns the cells which were joined during this call.
    ///
    /// NB: this could take as long as JOIN_NETWORK_TIMEOUT, which is significant.
    ///   Be careful to only await this future if it's important that cells be
    ///   joined before proceeding.
    async fn join_all_pending_cells(&self) -> Vec<CellId> {
        // Join the network but ignore errors because the
        // space retries joining all cells every 5 minutes.

        let pending_cells: Vec<(CellId, HolochainP2pCell)> = self
            .conductor
            .mark_pending_cells_as_joining()
            .into_iter()
            .map(|(id, cell)| (id, cell.holochain_p2p_cell().clone()))
            .collect();

        let tasks = pending_cells.into_iter()
            .map(|(cell_id, network)| async move {
                match tokio::time::timeout(JOIN_NETWORK_TIMEOUT, network.join()).await {
                    Ok(Err(e)) => {
                        tracing::info!(error = ?e, cell_id = ?cell_id, "Error while trying to join the network");
                        Err(cell_id)
                    }
                    Err(_) => {
                        tracing::info!(cell_id = ?cell_id, "Timed out trying to join the network");
                        Err(cell_id)
                    }
                    Ok(Ok(_)) => Ok(cell_id),
                }
            });

        let maybes: Vec<_> = futures::stream::iter(tasks)
            .buffer_unordered(100)
            .collect()
            .await;

        let (cell_ids, failed_joins): (Vec<_>, Vec<_>) =
            maybes.into_iter().partition(Result::is_ok);

        // These unwraps are both safe because of the partition.
        let cell_ids: Vec<_> = cell_ids.into_iter().map(Result::unwrap).collect();
        let failed_joins: Vec<_> = failed_joins.into_iter().map(Result::unwrap_err).collect();

        // Update the status of the cells which were able to join the network
        // (may or may not be all cells which were added)
        self.conductor
            .update_cell_status(cell_ids.as_slice(), CellStatus::Joined);

        self.conductor
            .update_cell_status(failed_joins.as_slice(), CellStatus::PendingJoin);

        cell_ids
    }

    pub(super) fn p2p_env(&self, space: Arc<KitsuneSpace>) -> EnvWrite {
        let mut p2p_env = self.p2p_env.lock();
        let db_sync_level = self.db_sync_level;
        p2p_env
            .entry(space.clone())
            .or_insert_with(move || {
                let root_env_dir = self.root_env_dir.as_ref();
                let keystore = self.keystore.clone();
                EnvWrite::open_with_sync_level(
                    root_env_dir,
                    DbKind::P2pAgentStore(space),
                    keystore,
                    db_sync_level,
                )
                .expect("failed to open p2p_agent_store database")
            })
            .clone()
    }

    pub(super) fn p2p_batch_sender(
        &self,
        space: Arc<KitsuneSpace>,
    ) -> tokio::sync::mpsc::Sender<P2pBatch> {
        let mut p2p_env = self.p2p_batch_senders.lock();
        p2p_env
            .entry(space.clone())
            .or_insert_with(|| {
                let (tx, rx) = tokio::sync::mpsc::channel(100);
                let env = { self.p2p_env(space) };
                tokio::spawn(p2p_agent_store::p2p_put_all_batch(env, rx));
                tx
            })
            .clone()
    }

    pub(super) fn p2p_metrics_env(&self, space: Arc<KitsuneSpace>) -> EnvWrite {
        let mut p2p_metrics_env = self.p2p_metrics_env.lock();
        let db_sync_level = self.db_sync_level;
        p2p_metrics_env
            .entry(space.clone())
            .or_insert_with(move || {
                let root_env_dir = self.root_env_dir.as_ref();
                let keystore = self.keystore.clone();
                EnvWrite::open_with_sync_level(
                    root_env_dir,
                    DbKind::P2pMetrics(space),
                    keystore,
                    db_sync_level,
                )
                .expect("failed to open p2p_metrics database")
            })
            .clone()
    }
}
