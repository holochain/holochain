#![deny(missing_docs)]
//! A Conductor is a dynamically changing group of [Cell]s.
//!
//! A Conductor can be managed:
//! - externally, via a [AppInterfaceApi]
//! - from within a [Cell], via [CellConductorApi]
//!
//! In normal use cases, a single Holochain user runs a single Conductor in a single process.
//! However, there's no reason we can't have multiple Conductors in a single process, simulating multiple
//! users in a testing environment.
use super::api::RealAppInterfaceApi;
use super::config::AdminInterfaceConfig;
use super::config::InterfaceDriver;
use super::dna_store::RealDnaStore;
use super::entry_def_store::get_entry_defs;
use super::error::ConductorError;
use super::handle::ConductorHandleImpl;
use super::interface::error::InterfaceResult;
use super::interface::websocket::spawn_admin_interface_task;
use super::interface::websocket::spawn_app_interface_task;
use super::interface::websocket::spawn_websocket_listener;
use super::interface::websocket::SIGNAL_BUFFER_SIZE;
use super::interface::SignalBroadcaster;
use super::manager::keep_alive_task;
use super::manager::ManagedTaskAdd;
use super::manager::ManagedTaskHandle;
use super::manager::TaskManagerRunHandle;
use super::paths::EnvironmentRootPath;
use super::state::AppInterfaceId;
use super::state::ConductorState;
use super::CellError;
use super::{api::CellConductorApi, state::AppInterfaceConfig};
use super::{api::CellConductorApiT, interface::AppInterfaceRuntime};
use super::{api::RealAdminInterfaceApi, manager::TaskManagerClient};
use crate::conductor::cell::error::CellResult;
use crate::conductor::cell::Cell;
use crate::conductor::config::ConductorConfig;
use crate::conductor::error::ConductorResult;
use crate::conductor::handle::ConductorHandle;
use crate::core::queue_consumer::InitialQueueTriggers;
use crate::{
    conductor::api::error::ConductorApiResult, core::ribosome::real_ribosome::RealRibosome,
};
pub use builder::*;
use futures::future;
use futures::future::TryFutureExt;
use futures::stream::StreamExt;
use holo_hash::DnaHash;
use holochain_conductor_api::conductor::PassphraseServiceConfig;
use holochain_conductor_api::AppStatusFilter;
use holochain_conductor_api::InstalledAppInfo;
use holochain_conductor_api::IntegrationStateDump;
use holochain_keystore::lair_keystore::spawn_lair_keystore;
use holochain_keystore::test_keystore::spawn_test_keystore;
use holochain_keystore::KeystoreSender;
use holochain_keystore::KeystoreSenderExt;
use holochain_sqlite::db::DbKind;
use holochain_sqlite::prelude::*;
use holochain_state::mutations;
use holochain_state::prelude::from_blob;
use holochain_state::prelude::StateMutationResult;
use holochain_types::prelude::*;
use rusqlite::OptionalExtension;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::*;

#[cfg(any(test, feature = "test_utils"))]
use super::handle::MockConductorHandleT;

/// The status of an installed Cell, which captures different phases of its lifecycle
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellStatus {
    /// Kitsune knows about this Cell and it is considered fully "online"
    Joined,
    /// The Cell is on its way to being fully joined. It is a valid Cell from
    /// the perspective of the conductor, and can handle HolochainP2pEvents,
    /// but it is considered not to be fully running from the perspective of
    /// app status, i.e. if any app has a required Cell with this status,
    /// the app is considered to be in the Paused state.
    PendingJoin,
}

/// Declarative filter for CellStatus
pub type CellStatusFilter = CellStatus;

/// A [`Cell`] tracked by a Conductor, along with its [`CellStatus`]
struct CellItem<CA>
where
    CA: CellConductorApiT,
{
    cell: Arc<Cell<CA>>,
    status: CellStatus,
}

impl<CA> CellItem<CA>
where
    CA: CellConductorApiT,
{
    pub fn is_running(&self) -> bool {
        self.status == CellStatus::Joined
    }

    pub fn is_pending(&self) -> bool {
        self.status == CellStatus::PendingJoin
    }
}

pub(crate) type StopBroadcaster = tokio::sync::broadcast::Sender<()>;
pub(crate) type StopReceiver = tokio::sync::broadcast::Receiver<()>;

/// A Conductor is a group of [Cell]s
pub struct Conductor<DS = RealDnaStore, CA = CellConductorApi>
where
    DS: DnaStore,
    CA: CellConductorApiT,
{
    /// The collection of cells associated with this Conductor
    cells: HashMap<CellId, CellItem<CA>>,

    /// The database for persisting state related to this Conductor
    conductor_env: EnvWrite,

    /// A database for storing wasm
    wasm_env: EnvWrite,

    /// The caches databases. These are shared across cells.
    /// There is one per unique Dna.
    caches: parking_lot::Mutex<HashMap<DnaHash, EnvWrite>>,

    /// Set to true when `conductor.shutdown()` has been called, so that other
    /// tasks can check on the shutdown status
    shutting_down: bool,

    /// The admin websocket ports this conductor has open.
    /// This exists so that we can run tests and bind to port 0, and find out
    /// the dynamically allocated port later.
    admin_websocket_ports: Vec<u16>,

    /// Collection app interface data, keyed by id
    app_interfaces: HashMap<AppInterfaceId, AppInterfaceRuntime>,

    /// The channels and handles needed to interact with the task_manager task.
    /// If this is None, then the task manager has not yet been initialized.
    pub(super) task_manager: Option<TaskManagerClient>,

    /// Placeholder for what will be the real DNA/Wasm cache
    dna_store: DS,

    /// Access to private keys for signing and encryption.
    keystore: KeystoreSender,

    /// The root environment directory where all environments are created
    root_env_dir: EnvironmentRootPath,

    /// Handle to the network actor.
    holochain_p2p: holochain_p2p::HolochainP2pRef,
}

impl Conductor {
    /// Create a conductor builder
    pub fn builder() -> ConductorBuilder {
        ConductorBuilder::new()
    }
}

//-----------------------------------------------------------------------------
// Public methods
//-----------------------------------------------------------------------------
impl<DS> Conductor<DS>
where
    DS: DnaStore + 'static,
{
    /// Returns a port which is guaranteed to have a websocket listener with an Admin interface
    /// on it. Useful for specifying port 0 and letting the OS choose a free port.
    pub fn get_arbitrary_admin_websocket_port(&self) -> Option<u16> {
        self.admin_websocket_ports.get(0).copied()
    }
}

//-----------------------------------------------------------------------------
/// Methods used by the [ConductorHandle]
//-----------------------------------------------------------------------------
impl<DS> Conductor<DS>
where
    DS: DnaStore + 'static,
{
    pub(super) fn cell_by_id(&self, cell_id: &CellId) -> ConductorResult<Arc<Cell>> {
        let item = self
            .cells
            .get(cell_id)
            .ok_or_else(|| ConductorError::CellMissing(cell_id.clone()))?;
        Ok(item.cell.clone())
    }

    /// Iterator over all cells, regardless of status.
    pub(super) fn _all_cells(&self) -> impl Iterator<Item = (&CellId, &Cell)> {
        self.cells.iter().map(|(id, item)| (id, item.cell.as_ref()))
    }

    /// Iterator over only the cells which are fully running. Generally used
    /// to handle conductor interface requests
    pub(super) fn running_cells(&self) -> impl Iterator<Item = (&CellId, &Cell)> {
        self.cells.iter().filter_map(|(id, item)| {
            if item.is_running() {
                Some((id, item.cell.as_ref()))
            } else {
                None
            }
        })
    }

    /// Iterator over only the cells which are pending validation. Used to
    /// discover which cells need to be joined to the network.
    pub(super) fn pending_cells(&self) -> impl Iterator<Item = (&CellId, &Cell)> {
        self.cells.iter().filter_map(|(id, item)| {
            if item.is_pending() {
                Some((id, item.cell.as_ref()))
            } else {
                None
            }
        })
    }

    /// A gate to put at the top of public functions to ensure that work is not
    /// attempted after a shutdown has been issued
    pub(super) fn check_running(&self) -> ConductorResult<()> {
        if self.shutting_down {
            Err(ConductorError::ShuttingDown)
        } else {
            Ok(())
        }
    }

    pub(super) fn dna_store(&self) -> &DS {
        &self.dna_store
    }

    pub(super) fn dna_store_mut(&mut self) -> &mut DS {
        &mut self.dna_store
    }

    /// Broadcasts the shutdown signal to all managed tasks.
    /// To actually wait for these tasks to complete, be sure to
    /// `take_shutdown_handle` to await for completion.
    pub(super) fn shutdown(&mut self) {
        self.shutting_down = true;
        if let Some(manager) = &self.task_manager {
            tracing::info!(
                "Sending shutdown signal to {} managed tasks.",
                manager.task_stop_broadcaster().receiver_count(),
            );
            manager
                .task_stop_broadcaster()
                .send(())
                .map(|_| ())
                .unwrap_or_else(|e| {
                    error!(?e, "Couldn't broadcast stop signal to managed tasks!");
                })
        }
    }

    /// Return the handle which waits for the task manager task to complete
    pub(super) fn take_shutdown_handle(&mut self) -> Option<TaskManagerRunHandle> {
        self.task_manager
            .as_mut()
            .and_then(|manager| manager.take_handle())
    }

    /// Spawn all admin interface tasks, register them with the TaskManager,
    /// and modify the conductor accordingly, based on the config passed in
    pub(super) async fn add_admin_interfaces_via_handle(
        &mut self,
        configs: Vec<AdminInterfaceConfig>,
        handle: ConductorHandle,
    ) -> ConductorResult<()>
    where
        DS: DnaStore + 'static,
    {
        let admin_api = RealAdminInterfaceApi::new(handle);
        let stop_tx = self
            .task_manager
            .as_ref()
            .expect("Task manager not started yet")
            .task_stop_broadcaster()
            .clone();

        // Closure to process each admin config item
        let spawn_from_config = |AdminInterfaceConfig { driver, .. }| {
            let admin_api = admin_api.clone();
            let stop_tx = stop_tx.clone();
            async move {
                match driver {
                    InterfaceDriver::Websocket { port } => {
                        let (listener_handle, listener) = spawn_websocket_listener(port).await?;
                        let port = listener_handle.local_addr().port().unwrap_or(port);
                        let handle: ManagedTaskHandle = spawn_admin_interface_task(
                            listener_handle,
                            listener,
                            admin_api.clone(),
                            stop_tx.subscribe(),
                        )?;
                        InterfaceResult::Ok((port, handle))
                    }
                }
            }
        };

        // spawn interface tasks, collect their JoinHandles,
        // panic on errors.
        let handles: Result<Vec<_>, _> =
            future::join_all(configs.into_iter().map(spawn_from_config))
                .await
                .into_iter()
                .collect();
        // Exit if the admin interfaces fail to be created
        let handles = handles.map_err(Box::new)?;

        {
            let mut ports = Vec::new();

            // First, register the keepalive task, to ensure the conductor doesn't shut down
            // in the absence of other "real" tasks
            self.manage_task(ManagedTaskAdd::ignore(
                tokio::spawn(keep_alive_task(stop_tx.subscribe())),
                "keepalive task",
            ))
            .await?;

            // Now that tasks are spawned, register them with the TaskManager
            for (port, handle) in handles {
                ports.push(port);
                self.manage_task(ManagedTaskAdd::ignore(
                    handle,
                    &format!("admin interface, port {}", port),
                ))
                .await?
            }
            for p in ports {
                self.add_admin_port(p);
            }
        }
        Ok(())
    }

    pub(super) async fn add_app_interface_via_handle(
        &mut self,
        port: either::Either<u16, AppInterfaceId>,
        handle: ConductorHandle,
    ) -> ConductorResult<u16> {
        let interface_id = match port {
            either::Either::Left(port) => AppInterfaceId::new(port),
            either::Either::Right(id) => id,
        };
        let port = interface_id.port();
        tracing::debug!("Attaching interface {}", port);
        let app_api = RealAppInterfaceApi::new(handle, interface_id.clone());
        // This receiver is thrown away because we can produce infinite new
        // receivers from the Sender
        let (signal_tx, _r) = tokio::sync::broadcast::channel(SIGNAL_BUFFER_SIZE);
        let stop_rx = self
            .task_manager
            .as_ref()
            .expect("Task manager not initialized")
            .task_stop_broadcaster()
            .subscribe();
        let (port, task) = spawn_app_interface_task(port, app_api, signal_tx.clone(), stop_rx)
            .await
            .map_err(Box::new)?;
        // TODO: RELIABILITY: Handle this task by restarting it if it fails and log the error
        self.manage_task(ManagedTaskAdd::ignore(
            task,
            &format!("app interface, port {}", port),
        ))
        .await?;
        let interface = AppInterfaceRuntime::Websocket { signal_tx };

        if self.app_interfaces.contains_key(&interface_id) {
            return Err(ConductorError::AppInterfaceIdCollision(interface_id));
        }

        self.app_interfaces.insert(interface_id.clone(), interface);
        let config = AppInterfaceConfig::websocket(port);
        self.update_state(|mut state| {
            state.app_interfaces.insert(interface_id, config);
            Ok(state)
        })
        .await?;
        tracing::debug!("App interface added at port: {}", port);
        Ok(port)
    }

    pub(super) async fn list_app_interfaces(&self) -> ConductorResult<Vec<u16>> {
        Ok(self
            .get_state()
            .await?
            .app_interfaces
            .values()
            .map(|config| config.driver.port())
            .collect())
    }

    pub(super) async fn register_dna_wasm(
        &self,
        dna: DnaFile,
    ) -> ConductorResult<Vec<(EntryDefBufferKey, EntryDef)>> {
        let is_full_wasm_dna = dna
            .dna_def()
            .zomes
            .iter()
            .all(|(_, zome_def)| matches!(zome_def, ZomeDef::Wasm(_)));

        // Only install wasm if the DNA is composed purely of WasmZomes (no InlineZomes)
        if is_full_wasm_dna {
            Ok(self.put_wasm(dna.clone()).await?)
        } else {
            Ok(Vec::with_capacity(0))
        }
    }

    pub(super) async fn register_dna_entry_defs(
        &mut self,
        entry_defs: Vec<(EntryDefBufferKey, EntryDef)>,
    ) -> ConductorResult<()> {
        self.dna_store_mut().add_entry_defs(entry_defs);
        Ok(())
    }

    pub(super) async fn register_phenotype(&mut self, dna: DnaFile) -> ConductorResult<()> {
        self.dna_store_mut().add_dna(dna);
        Ok(())
    }

    /// Start all app interfaces currently in state.
    /// This should only be run at conductor initialization.
    #[allow(irrefutable_let_patterns)]
    pub(super) async fn startup_app_interfaces_via_handle(
        &mut self,
        handle: ConductorHandle,
    ) -> ConductorResult<()> {
        for id in self.get_state().await?.app_interfaces.keys().cloned() {
            tracing::debug!("Starting up app interface: {:?}", id);
            let _ = self
                .add_app_interface_via_handle(either::Right(id), handle.clone())
                .await?;
        }
        Ok(())
    }

    pub(super) fn signal_broadcaster(&self) -> SignalBroadcaster {
        SignalBroadcaster::new(
            self.app_interfaces
                .values()
                .map(|i| i.signal_tx())
                .cloned()
                .collect(),
        )
    }

    /// Instantiate a Ribosome for use with a DNA
    pub(crate) fn get_ribosome(&self, dna_hash: &DnaHash) -> ConductorResult<RealRibosome> {
        match self.dna_store().get(dna_hash) {
            Some(dna) => Ok(RealRibosome::new(dna)),
            None => Err(DnaError::DnaMissing(dna_hash.to_owned()).into()),
        }
    }

    fn get_or_create_cache(&self, dna_hash: &DnaHash) -> ConductorResult<EnvWrite> {
        let mut caches = self.caches.lock();
        match caches.get(dna_hash) {
            Some(env) => Ok(env.clone()),
            None => {
                let dir = self.root_env_dir.clone();
                let env = EnvWrite::open(
                    dir.as_ref(),
                    DbKind::Cache(dna_hash.clone()),
                    self.keystore.clone(),
                )?;
                caches.insert(dna_hash.clone(), env.clone());
                Ok(env)
            }
        }
    }

    /// Adjust app statuses (via state transitions) to match the current
    /// reality of which Cells are present in the conductor.
    /// - Do not change state for Disabled apps. For all others:
    /// - If an app is Paused but all of its (required) Cells are on,
    ///     then set it to Running
    /// - If an app is Running but at least one of its (required) Cells are off,
    ///     then set it to Paused
    pub(super) async fn reconcile_app_status_with_cell_status<S>(
        &mut self,
        app_ids: Option<S>,
    ) -> ConductorResult<AppStatusFx>
    where
        S: Into<HashSet<InstalledAppId>>,
    {
        use AppStatus::*;
        use AppStatusTransition::*;

        let app_ids: Option<HashSet<InstalledAppId>> = app_ids.map(S::into);
        let running_cells: HashSet<CellId> = self.running_cells().map(first).cloned().collect();
        let (_, delta) = self
            .update_state_prime(move |mut state| {
                let apps = state.installed_apps_mut().iter_mut().filter(|(id, _)| {
                    app_ids
                        .as_ref()
                        .map(|ids| ids.contains(&**id))
                        .unwrap_or(true)
                });
                let delta = apps
                    .into_iter()
                    .map(|(_app_id, app)| {
                        match app.status().clone() {
                            Running => {
                                // If not all required cells are running, pause the app
                                let missing: Vec<_> = app
                                    .required_cells()
                                    .filter(|id| !running_cells.contains(id))
                                    .collect();
                                if !missing.is_empty() {
                                    let reason = PausedAppReason::Error(format!(
                                        "Some cells are missing / not able to run: {:#?}",
                                        missing
                                    ));
                                    app.status.transition(Pause(reason))
                                } else {
                                    AppStatusFx::NoChange
                                }
                            }
                            Paused(_) => {
                                // If all required cells are now running, restart the app
                                if app.required_cells().all(|id| running_cells.contains(id)) {
                                    app.status.transition(Start)
                                } else {
                                    AppStatusFx::NoChange
                                }
                            }
                            Disabled(_) => {
                                // Disabled status should never automatically change.
                                AppStatusFx::NoChange
                            }
                        }
                    })
                    .fold(AppStatusFx::default(), AppStatusFx::combine);
                Ok((state, delta))
            })
            .await?;
        Ok(delta)
    }

    /// Remove all Cells which are not referenced by any Enabled app.
    /// (Cells belonging to Paused apps are not considered "dangling" and will not be removed)
    pub(super) async fn remove_dangling_cells(&mut self) -> ConductorResult<()> {
        let state = self.get_state().await?;
        let keepers: HashSet<CellId> = state
            .enabled_apps()
            .flat_map(|(_, app)| app.all_cells().cloned().collect::<HashSet<_>>())
            .collect();

        // Clean up all cells that will be dropped (leave network, etc.)
        let tasks = self
            .cells
            .iter()
            .filter(|(id, _)| !keepers.contains(id))
            .map(|(_, cell)| cell.cell.cleanup());
        futures::future::join_all(tasks)
            .await
            .into_iter()
            .collect::<CellResult<Vec<()>>>()?;

        // drop all but the keepers
        self.cells.retain(|id, _| keepers.contains(id));
        Ok(())
    }

    /// Attempt to create all necessary Cells which have not already been created
    /// and added to the conductor, namely the cells which are referenced by
    /// Running apps. If there are no cells to create, this function does nothing.
    ///
    /// Returns a Result for each attempt so that successful creations can be
    /// handled alongside the failures.
    pub(super) async fn create_cells_for_running_apps(
        &self,
        conductor_handle: ConductorHandle,
    ) -> ConductorResult<Vec<Result<(Cell, InitialQueueTriggers), (CellId, CellError)>>> {
        // Data required to create apps
        let root_env_dir = PathBuf::from(self.root_env_dir.clone());
        let keystore = self.keystore.clone();
        let task_manager = self
            .task_manager
            .as_ref()
            .expect("Task manager not initialized");

        // Closure for creating all cells in an app
        let state = self.get_state().await?;

        // Collect all CellIds across all apps, deduped
        let app_cells: HashSet<&CellId> = state
            .installed_apps()
            .iter()
            .filter(|(_, app)| app.status().is_running())
            .flat_map(|(_id, app)| app.all_cells().collect::<Vec<&CellId>>())
            .collect();

        // calculate the existing cells so we can filter those out, only creating
        // cells for CellIds that don't have cells
        let on_cells: HashSet<&CellId> = self.cells.iter().map(first).collect();

        let tasks = app_cells.difference(&on_cells).map(|&cell_id| {
            let root_env_dir = root_env_dir.clone();
            let keystore = keystore.clone();
            let conductor_handle = conductor_handle.clone();
            async move {
                use holochain_p2p::actor::HolochainP2pRefToCell;
                let holochain_p2p_cell = self
                    .holochain_p2p
                    .to_cell(cell_id.dna_hash().clone(), cell_id.agent_pubkey().clone());

                let env = EnvWrite::open(
                    &root_env_dir,
                    DbKind::Cell(cell_id.clone()),
                    keystore.clone(),
                )
                .map_err(|err| (cell_id.clone(), err.into()))?;

                let cache = self
                    .get_or_create_cache(cell_id.dna_hash())
                    .map_err(|e| CellError::FailedToCreateCache(e.into()))
                    .map_err(|err| (cell_id.clone(), err))?;

                Cell::create(
                    cell_id.clone(),
                    conductor_handle.clone(),
                    env,
                    cache,
                    holochain_p2p_cell,
                    task_manager.task_add_sender().clone(),
                    task_manager.task_stop_broadcaster().clone(),
                )
                .await
                .map_err(|err| (cell_id.clone(), err))
            }
        });

        // Join on all apps and return a list of
        // apps that had succelly created cells
        // and any apps that encounted errors
        Ok(futures::future::join_all(tasks).await)
    }

    /// Register an app as disabled in the database
    pub(super) async fn add_disabled_app_to_db(
        &mut self,
        app: InstalledAppCommon,
    ) -> ConductorResult<StoppedApp> {
        let (_, stopped_app) = self
            .update_state_prime(move |mut state| {
                let stopped_app = state.add_app(app)?;
                Ok((state, stopped_app))
            })
            .await?;
        Ok(stopped_app)
    }

    /// Transition an app's status to a new state.
    #[tracing::instrument(skip(self))]
    pub(super) async fn transition_app_status(
        &mut self,
        app_id: &InstalledAppId,
        transition: AppStatusTransition,
    ) -> ConductorResult<(InstalledApp, AppStatusFx)> {
        Ok(self
            .update_state_prime(move |mut state| {
                let (app, delta) = state.transition_app_status(&app_id, transition)?.clone();
                let app = app.clone();
                Ok((state, (app, delta)))
            })
            .await?
            .1)
    }

    /// Entirely remove an app from the database, returning the removed app.
    pub(super) async fn remove_app_from_db(
        &mut self,
        app_id: &InstalledAppId,
    ) -> ConductorResult<InstalledApp> {
        let (_state, app) = self
            .update_state_prime({
                let app_id = app_id.clone();
                move |mut state| {
                    let app = state.remove_app(&app_id)?;
                    Ok((state, app))
                }
            })
            .await?;
        Ok(app)
    }

    /// Add fully constructed cells to the cell map in the Conductor
    pub(super) fn add_and_initialize_cells(&mut self, cells: Vec<(Cell, InitialQueueTriggers)>) {
        let (cells, triggers): (Vec<_>, Vec<_>) = cells.into_iter().unzip();
        for cell in cells {
            let cell_id = cell.id().clone();
            tracing::info!(?cell_id, "ADD CELL");
            self.cells.insert(
                cell_id,
                CellItem {
                    cell: Arc::new(cell),
                    status: CellStatus::PendingJoin,
                },
            );
        }
        for trigger in triggers {
            trigger.initialize_workflows();
        }
    }

    /// Change the CellStatus of the given Cells in the Conductor.
    /// Silently ignores Cells that don't exist.
    pub(super) fn update_cell_status(&mut self, cell_ids: &[CellId], status: CellStatus) {
        for cell_id in cell_ids {
            if let Some(mut cell) = self.cells.get_mut(cell_id) {
                cell.status = status.clone();
            }
        }
    }

    /// Associate a Cell with an existing App
    pub(super) async fn add_clone_cell_to_app(
        &mut self,
        app_id: &InstalledAppId,
        slot_id: &SlotId,
        properties: YamlProperties,
    ) -> ConductorResult<CellId> {
        let dna_store = &self.dna_store;
        let (_, child_dna) = self
            .update_state_prime(move |mut state| {
                if let Some(app) = state.installed_apps_mut().get_mut(app_id) {
                    let slot = app
                        .slots()
                        .get(slot_id)
                        .ok_or_else(|| AppError::SlotIdMissing(slot_id.to_owned()))?;
                    let parent_dna_hash = slot.dna_hash();
                    let dna = dna_store
                        .get(parent_dna_hash)
                        .ok_or_else(|| DnaError::DnaMissing(parent_dna_hash.to_owned()))?
                        .modify_phenotype(random_uid(), properties)?;
                    Ok((state, dna))
                } else {
                    Err(ConductorError::AppNotRunning(app_id.clone()))
                }
            })
            .await?;
        let child_dna_hash = child_dna.dna_hash().to_owned();
        self.register_phenotype(child_dna).await?;
        let (_, cell_id) = self
            .update_state_prime(|mut state| {
                if let Some(app) = state.installed_apps_mut().get_mut(app_id) {
                    let agent_key = app.slot(slot_id)?.agent_key().to_owned();
                    let cell_id = CellId::new(child_dna_hash, agent_key);
                    app.add_clone(slot_id, cell_id.clone())?;
                    Ok((state, cell_id))
                } else {
                    Err(ConductorError::AppNotRunning(app_id.clone()))
                }
            })
            .await?;
        Ok(cell_id)
    }

    pub(super) async fn load_wasms_into_dna_files(
        &self,
    ) -> ConductorResult<(
        impl IntoIterator<Item = (DnaHash, DnaFile)>,
        impl IntoIterator<Item = (EntryDefBufferKey, EntryDef)>,
    )> {
        let env = &self.wasm_env;

        // Load out all dna defs
        let (wasm_tasks, defs) = env
            .async_reader(move |txn| {
                // Get all the dna defs.
                let dna_defs: Vec<_> = holochain_state::dna_def::get_all(&txn)?
                    .into_iter()
                    .collect();

                // Gather all the unique wasms.
                let unique_wasms = dna_defs
                    .iter()
                    .flat_map(|dna_def| {
                        dna_def
                            .zomes
                            .iter()
                            .map(|(zome_name, zome)| Ok(zome.wasm_hash(&zome_name)?))
                    })
                    .collect::<ConductorResult<HashSet<_>>>()?;

                // Get the code for each unique wasm.
                let wasms = unique_wasms
                    .into_iter()
                    .map(|wasm_hash| {
                        holochain_state::wasm::get(&txn, &wasm_hash)?
                            .map(|hashed| hashed.into_content())
                            .ok_or(ConductorError::WasmMissing)
                            .map(|wasm| (wasm_hash, wasm))
                    })
                    .collect::<ConductorResult<HashMap<_, _>>>()?;
                let wasm_tasks =
                    holochain_state::dna_def::get_all(&txn)?
                        .into_iter()
                        .map(|dna_def| {
                            // Load all wasms for each dna_def from the wasm db into memory
                            let wasms = dna_def.zomes.clone().into_iter().filter_map(
                                |(zome_name, zome)| {
                                    let wasm_hash = zome.wasm_hash(&zome_name).ok()?;
                                    // Note this is a cheap arc clone.
                                    wasms.get(&wasm_hash).cloned()
                                },
                            );
                            let wasms = wasms.collect::<Vec<_>>();
                            async move {
                                let dna_file = DnaFile::new(dna_def.into_content(), wasms).await?;
                                ConductorResult::Ok((dna_file.dna_hash().clone(), dna_file))
                            }
                        })
                        // This needs to happen due to the environment not being Send
                        .collect::<Vec<_>>();
                let defs = holochain_state::entry_def::get_all(&txn)?;
                ConductorResult::Ok((wasm_tasks, defs))
            })
            .await?;
        // try to join all the tasks and return the list of dna files
        let dnas = futures::future::try_join_all(wasm_tasks).await?;
        Ok((dnas, defs))
    }

    /// Get the root environment directory.
    pub fn root_env_dir(&self) -> &EnvironmentRootPath {
        &self.root_env_dir
    }

    /// Get the keystore.
    pub fn keystore(&self) -> &KeystoreSender {
        &self.keystore
    }

    /// Remove cells from the cell map in the Conductor
    pub(super) async fn remove_cells(&mut self, cell_ids: Vec<CellId>) {
        for cell_id in cell_ids {
            if let Some(item) = self.cells.remove(&cell_id) {
                // TODO: make this function async, or ensure that cleanup is fast,
                //       so we don't hold the conductor lock unduly long while doing cleanup
                //       (currently cleanup is a noop, so it's not urgent)
                if let Err(err) = item.cell.cleanup().await {
                    tracing::error!("Error cleaning up Cell: {:?}\nCellId: {}", err, cell_id);
                }
            }
        }
    }

    /// Restart every paused app
    pub(super) async fn start_paused_apps(&mut self) -> ConductorResult<AppStatusFx> {
        let (_, delta) = self
            .update_state_prime(|mut state| {
                let ids = state.paused_apps().map(first).cloned().collect::<Vec<_>>();
                if !ids.is_empty() {
                    tracing::info!("Restarting {} paused apps: {:#?}", ids.len(), ids);
                }
                let deltas: Vec<AppStatusFx> = ids
                    .into_iter()
                    .map(|id| {
                        state
                            .transition_app_status(&id, AppStatusTransition::Start)
                            .map(second)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let delta = deltas
                    .into_iter()
                    .fold(AppStatusFx::default(), AppStatusFx::combine);
                Ok((state, delta))
            })
            .await?;
        Ok(delta)
    }

    pub(super) async fn put_wasm(
        &self,
        dna: DnaFile,
    ) -> ConductorResult<Vec<(EntryDefBufferKey, EntryDef)>> {
        let env = self.wasm_env.clone();

        let zome_defs = get_entry_defs(dna.clone())?;

        // TODO: PERF: This loop might be slow
        let wasms = futures::future::join_all(
            dna.code()
                .clone()
                .into_iter()
                .map(|(_, dna_wasm)| DnaWasmHashed::from_content(dna_wasm)),
        )
        .await;

        env.async_commit({
            let zome_defs = zome_defs.clone();
            move |txn| {
                for dna_wasm in wasms {
                    if !holochain_state::wasm::contains(txn, dna_wasm.as_hash())? {
                        holochain_state::wasm::put(txn, dna_wasm)?;
                    }
                }

                for (key, entry_def) in zome_defs.clone() {
                    holochain_state::entry_def::put(txn, key, entry_def)?;
                }

                if !holochain_state::dna_def::contains(txn, dna.dna_hash())? {
                    holochain_state::dna_def::put(txn, dna.dna_def().clone())?;
                }
                StateMutationResult::Ok(())
            }
        })
        .await?;

        Ok(zome_defs)
    }

    pub(super) async fn list_cell_ids(
        &self,
        filter: Option<CellStatusFilter>,
    ) -> ConductorResult<Vec<CellId>> {
        Ok(self
            .cells
            .iter()
            .filter_map(|(id, cell)| {
                let matches = filter
                    .as_ref()
                    .map(|status| cell.status == *status)
                    .unwrap_or(true);
                if matches {
                    Some(id)
                } else {
                    None
                }
            })
            .cloned()
            .collect())
    }

    pub(super) async fn list_running_apps(&self) -> ConductorResult<Vec<InstalledAppId>> {
        let state = self.get_state().await?;
        Ok(state.running_apps().map(|(id, _)| id).cloned().collect())
    }

    pub(super) async fn list_apps(
        &self,
        status_filter: Option<AppStatusFilter>,
    ) -> ConductorResult<Vec<InstalledAppInfo>> {
        use AppStatusFilter::*;
        let conductor_state = self.get_state().await?;

        let apps_ids: Vec<&String> = match status_filter {
            Some(Enabled) => conductor_state.enabled_apps().map(|(id, _)| id).collect(),
            Some(Disabled) => conductor_state.disabled_apps().map(|(id, _)| id).collect(),
            Some(Running) => conductor_state.running_apps().map(|(id, _)| id).collect(),
            Some(Stopped) => conductor_state.stopped_apps().map(|(id, _)| id).collect(),
            Some(Paused) => conductor_state.paused_apps().map(|(id, _)| id).collect(),
            None => conductor_state.installed_apps().keys().collect(),
        };

        let apps_info: Vec<InstalledAppInfo> = apps_ids
            .into_iter()
            .filter_map(|app_id| conductor_state.get_app_info(app_id))
            .collect();

        Ok(apps_info)
    }

    pub(super) async fn list_running_apps_for_cell_id(
        &self,
        cell_id: &CellId,
    ) -> ConductorResult<HashSet<InstalledAppId>> {
        Ok(self
            .get_state()
            .await?
            .running_apps()
            .filter(|(_, v)| v.all_cells().any(|i| i == cell_id))
            .map(|(k, _)| k)
            .cloned()
            .collect())
    }

    pub(super) fn print_setup(&self) {
        use std::fmt::Write;
        let mut out = String::new();
        for port in &self.admin_websocket_ports {
            writeln!(&mut out, "###ADMIN_PORT:{}###", port).expect("Can't write setup to std out");
        }
        println!("\n###HOLOCHAIN_SETUP###\n{}###HOLOCHAIN_SETUP_END###", out);
    }

    #[cfg(any(test, feature = "test_utils"))]
    pub(super) async fn get_state_from_handle(&self) -> ConductorResult<ConductorState> {
        self.get_state().await
    }

    #[cfg(any(test, feature = "test_utils"))]
    pub(super) async fn add_test_app_interface<I: Into<AppInterfaceId>>(
        &mut self,
        id: I,
    ) -> ConductorResult<()> {
        let id = id.into();
        let (signal_tx, _r) = tokio::sync::broadcast::channel(1000);
        if self.app_interfaces.contains_key(&id) {
            return Err(ConductorError::AppInterfaceIdCollision(id));
        }
        let _ = self
            .app_interfaces
            .insert(id, AppInterfaceRuntime::Test { signal_tx });
        Ok(())
    }
}

/// Perform Genesis on the source chains for each of the specified CellIds.
///
/// If genesis fails for any cell, this entire function fails, and all other
/// partial or complete successes are rolled back.
/// Note this function takes read locks so should not be called from within a read lock.
pub(super) async fn genesis_cells(
    root_env_dir: PathBuf,
    keystore: KeystoreSender,
    cell_ids_with_proofs: Vec<(CellId, Option<MembraneProof>)>,
    conductor_handle: ConductorHandle,
) -> ConductorResult<()> {
    let cells_tasks = cell_ids_with_proofs
        .into_iter()
        .map(|(cell_id, proof)| async {
            let root_env_dir = root_env_dir.clone();
            let keystore = keystore.clone();
            let conductor_handle = conductor_handle.clone();
            let cell_id_inner = cell_id.clone();
            let ribosome = conductor_handle
                .get_ribosome(cell_id.dna_hash())
                .await
                .map_err(Box::new)?;
            tokio::spawn(async move {
                let env = EnvWrite::open(
                    &root_env_dir,
                    DbKind::Cell(cell_id_inner.clone()),
                    keystore.clone(),
                )?;
                Cell::genesis(cell_id_inner, conductor_handle, env, ribosome, proof).await
            })
            .map_err(CellError::from)
            .and_then(|result| async move { result.map(|_| cell_id) })
            .await
        });
    let (success, errors): (Vec<_>, Vec<_>) = futures::future::join_all(cells_tasks)
        .await
        .into_iter()
        .partition(Result::is_ok);

    // unwrap safe because of the partition
    let success = success.into_iter().map(Result::unwrap);

    // If there were errors, cleanup and return the errors
    if !errors.is_empty() {
        for cell_id in success {
            let db = DbWrite::open(&root_env_dir, DbKind::Cell(cell_id))?;
            db.remove().await?;
        }

        // match needed to avoid Debug requirement on unwrap_err
        let errors = errors
            .into_iter()
            .map(|e| match e {
                Err(e) => e,
                Ok(_) => unreachable!("Safe because of the partition"),
            })
            .collect();

        Err(ConductorError::GenesisFailed { errors })
    } else {
        // No errors so return the cells
        Ok(())
    }
}

/// Dump the integration json state.
pub async fn integration_dump(vault: &EnvRead) -> ConductorApiResult<IntegrationStateDump> {
    vault
        .async_reader(move |txn| {
            let integrated = txn.query_row(
                "SELECT count(hash) FROM DhtOp WHERE when_integrated IS NOT NULL",
                [],
                |row| row.get(0),
            )?;
            let integration_limbo = txn.query_row(
            "SELECT count(hash) FROM DhtOp WHERE when_integrated IS NULL AND validation_stage = 3",
            [],
            |row| row.get(0),
        )?;
            let validation_limbo = txn.query_row(
                "
                SELECT count(hash) FROM DhtOp
                WHERE when_integrated IS NULL
                AND (
                    (is_authored = 1 AND validation_stage IS NOT NULL AND validation_stage < 3)
                    OR
                    (is_authored = 0 AND (validation_stage IS NULL OR validation_stage < 3))
                )
                ",
                [],
                |row| row.get(0),
            )?;
            ConductorApiResult::Ok(IntegrationStateDump {
                validation_limbo,
                integration_limbo,
                integrated,
            })
        })
        .await
}

//-----------------------------------------------------------------------------
// Private methods
//-----------------------------------------------------------------------------

impl<DS> Conductor<DS>
where
    DS: DnaStore + 'static,
{
    #[allow(clippy::too_many_arguments)]
    async fn new(
        env: EnvWrite,
        wasm_env: EnvWrite,
        dna_store: DS,
        keystore: KeystoreSender,
        root_env_dir: EnvironmentRootPath,
        holochain_p2p: holochain_p2p::HolochainP2pRef,
    ) -> ConductorResult<Self> {
        Ok(Self {
            conductor_env: env,
            wasm_env,
            caches: parking_lot::Mutex::new(HashMap::new()),
            cells: HashMap::new(),
            shutting_down: false,
            app_interfaces: HashMap::new(),
            task_manager: None,
            admin_websocket_ports: Vec::new(),
            dna_store,
            keystore,
            root_env_dir,
            holochain_p2p,
        })
    }

    pub(super) async fn get_state(&self) -> ConductorResult<ConductorState> {
        self.conductor_env.conn()?.with_reader(|txn| {
            let state = txn
                .query_row("SELECT blob FROM ConductorState WHERE id = 1", [], |row| {
                    row.get("blob")
                })
                .optional()?;
            let state = match state {
                Some(state) => from_blob(state)?,
                None => ConductorState::default(),
            };
            Ok(state)
        })
    }

    /// Update the internal state with a pure function mapping old state to new
    async fn update_state<F: Send>(&self, f: F) -> ConductorResult<ConductorState>
    where
        F: FnOnce(ConductorState) -> ConductorResult<ConductorState> + 'static,
    {
        let (state, _) = self.update_state_prime(|s| Ok((f(s)?, ()))).await?;
        Ok(state)
    }

    /// Update the internal state with a pure function mapping old state to new,
    /// which may also produce an output value which will be the output of
    /// this function
    async fn update_state_prime<F, O>(&self, f: F) -> ConductorResult<(ConductorState, O)>
    where
        F: FnOnce(ConductorState) -> ConductorResult<(ConductorState, O)>,
        O: Send,
    {
        self.check_running()?;
        let output = self
            .conductor_env
            .async_commit_in_place(move |txn| {
                let state = txn
                    .query_row("SELECT blob FROM ConductorState WHERE id = 1", [], |row| {
                        row.get("blob")
                    })
                    .optional()?;
                let state = match state {
                    Some(state) => from_blob(state)?,
                    None => ConductorState::default(),
                };
                let (new_state, output) = f(state)?;
                mutations::insert_conductor_state(txn, (&new_state).try_into()?)?;
                Result::<_, ConductorError>::Ok((new_state, output))
            })
            .await?;
        Ok(output)
    }

    fn add_admin_port(&mut self, port: u16) {
        self.admin_websocket_ports.push(port);
    }

    /// Sends a JoinHandle to the TaskManager task to be managed
    async fn manage_task(&mut self, handle: ManagedTaskAdd) -> ConductorResult<()> {
        self.task_manager
            .as_ref()
            .expect("Task manager not initialized")
            .task_add_sender()
            .send(handle)
            .await
            .map_err(|e| ConductorError::SubmitTaskError(format!("{}", e)))
    }
}

mod builder {
    use super::*;
    use crate::conductor::dna_store::RealDnaStore;
    use crate::conductor::handle::DevSettings;
    use crate::conductor::ConductorHandle;
    use holochain_sqlite::db::DbKind;
    #[cfg(any(test, feature = "test_utils"))]
    use holochain_state::test_utils::TestEnvs;

    /// A configurable Builder for Conductor and sometimes ConductorHandle
    #[derive(Default)]
    pub struct ConductorBuilder<DS = RealDnaStore> {
        /// The configuration
        pub config: ConductorConfig,
        /// The DnaStore (mockable)
        pub dna_store: DS,
        /// Optional keystore override
        pub keystore: Option<KeystoreSender>,
        #[cfg(any(test, feature = "test_utils"))]
        /// Optional state override (for testing)
        pub state: Option<ConductorState>,
        #[cfg(any(test, feature = "test_utils"))]
        /// Optional handle mock (for testing)
        pub mock_handle: Option<MockConductorHandleT>,
    }

    impl ConductorBuilder {
        /// Default ConductorBuilder
        pub fn new() -> Self {
            Self::default()
        }
    }

    impl ConductorBuilder<MockDnaStore> {
        /// ConductorBuilder using mocked DnaStore, for testing
        pub fn with_mock_dna_store(dna_store: MockDnaStore) -> ConductorBuilder<MockDnaStore> {
            Self {
                dna_store,
                ..Default::default()
            }
        }
    }

    impl<DS> ConductorBuilder<DS>
    where
        DS: DnaStore + 'static,
    {
        /// Set the ConductorConfig used to build this Conductor
        pub fn config(mut self, config: ConductorConfig) -> Self {
            self.config = config;
            self
        }

        /// Initialize a "production" Conductor
        pub async fn build(self) -> ConductorResult<ConductorHandle> {
            cfg_if::cfg_if! {
                // if mock_handle is specified, return that instead of
                // a real handle
                if #[cfg(test)] {
                    if let Some(handle) = self.mock_handle {
                        return Ok(Arc::new(handle));
                    }
                }
            }

            tracing::info!(?self.config);

            let keystore = if let Some(keystore) = self.keystore {
                keystore
            } else if self.config.use_dangerous_test_keystore {
                let keystore = spawn_test_keystore().await?;
                // pre-populate with our two fixture agent keypairs
                keystore
                    .generate_sign_keypair_from_pure_entropy()
                    .await
                    .unwrap();
                keystore
                    .generate_sign_keypair_from_pure_entropy()
                    .await
                    .unwrap();
                keystore
            } else {
                let passphrase = match &self.config.passphrase_service {
                    PassphraseServiceConfig::DangerInsecureFromConfig { passphrase } => {
                        tracing::warn!("USING INSECURE PASSPHRASE FROM CONFIG--This defeats the whole purpose of having a passphrase. (unfortunately, there isn't another option at the moment).");
                        // TODO - use `new_mem_locked` when we have a secure source
                        sodoken::BufRead::new_no_lock(passphrase.as_bytes())
                    }
                    oth => {
                        panic!("We don't support this passphrase_service yet: {:?}", oth);
                    }
                };

                spawn_lair_keystore(self.config.keystore_path.as_deref(), passphrase).await?
            };
            let env_path = self.config.environment_path.clone();

            let environment =
                EnvWrite::open(env_path.as_ref(), DbKind::Conductor, keystore.clone())?;

            let wasm_environment =
                EnvWrite::open(env_path.as_ref(), DbKind::Wasm, keystore.clone())?;

            #[cfg(any(test, feature = "test_utils"))]
            let state = self.state;

            let Self {
                dna_store, config, ..
            } = self;

            let network_config = match &config.network {
                None => holochain_p2p::kitsune_p2p::KitsuneP2pConfig::default(),
                Some(config) => config.clone(),
            };
            let (cert_digest, cert, cert_priv_key) =
                keystore.get_or_create_first_tls_cert().await?;
            let tls_config =
                holochain_p2p::kitsune_p2p::dependencies::kitsune_p2p_proxy::TlsConfig {
                    cert,
                    cert_priv_key,
                    cert_digest,
                };
            let (holochain_p2p, p2p_evt) =
                holochain_p2p::spawn_holochain_p2p(network_config, tls_config).await?;

            let conductor = Conductor::new(
                environment,
                wasm_environment,
                dna_store,
                keystore,
                env_path,
                holochain_p2p,
            )
            .await?;

            #[cfg(any(test, feature = "test_utils"))]
            let conductor = Self::update_fake_state(state, conductor).await?;

            // Get data before handle
            let keystore = conductor.keystore.clone();
            let holochain_p2p = conductor.holochain_p2p.clone();

            // Create handle
            let handle: ConductorHandle = Arc::new(ConductorHandleImpl {
                root_env_dir: config.environment_path.clone(),
                conductor: RwLock::new(conductor),
                keystore,
                holochain_p2p,
                p2p_env: Arc::new(parking_lot::Mutex::new(HashMap::new())),
                p2p_metrics_env: Arc::new(parking_lot::Mutex::new(HashMap::new())),

                #[cfg(any(test, feature = "test_utils"))]
                dev_settings: parking_lot::RwLock::new(DevSettings::default()),
            });

            Self::finish(handle, config, p2p_evt).await
        }

        async fn finish(
            handle: ConductorHandle,
            conductor_config: ConductorConfig,
            p2p_evt: holochain_p2p::event::HolochainP2pEventReceiver,
        ) -> ConductorResult<ConductorHandle> {
            tokio::task::spawn(p2p_event_task(p2p_evt, handle.clone()));

            let configs = conductor_config.admin_interfaces.unwrap_or_default();
            let cell_startup_errors = handle.clone().initialize_conductor(configs).await?;

            // TODO: This should probably be emitted over the admin interface
            if !cell_startup_errors.is_empty() {
                error!(
                    msg = "Failed to create the following active apps",
                    ?cell_startup_errors
                );
            }

            handle.print_setup().await;

            Ok(handle)
        }

        /// Pass a test keystore in, to ensure that generated test agents
        /// are actually available for signing (especially for tryorama compat)
        pub fn with_keystore(mut self, keystore: KeystoreSender) -> Self {
            self.keystore = Some(keystore);
            self
        }

        #[cfg(any(test, feature = "test_utils"))]
        /// Sets some fake conductor state for tests
        pub fn fake_state(mut self, state: ConductorState) -> Self {
            self.state = Some(state);
            self
        }

        /// Pass a mock handle in, which will be returned regardless of whatever
        /// else happens to this builder
        #[cfg(any(test, feature = "test_utils"))]
        pub fn with_mock_handle(mut self, handle: MockConductorHandleT) -> Self {
            self.mock_handle = Some(handle);
            self
        }

        #[cfg(any(test, feature = "test_utils"))]
        async fn update_fake_state(
            state: Option<ConductorState>,
            conductor: Conductor<DS>,
        ) -> ConductorResult<Conductor<DS>> {
            if let Some(state) = state {
                conductor.update_state(move |_| Ok(state)).await?;
            }
            Ok(conductor)
        }

        /// Build a Conductor with a test environment
        #[cfg(any(test, feature = "test_utils"))]
        pub async fn test(
            mut self,
            envs: &TestEnvs,
            extra_dnas: &[DnaFile],
        ) -> ConductorResult<ConductorHandle> {
            let keystore = envs.conductor().keystore().clone();

            self.config.environment_path = envs.path().to_path_buf().into();

            let (holochain_p2p, p2p_evt) =
                holochain_p2p::spawn_holochain_p2p(self.config.network.clone().unwrap_or_default(), holochain_p2p::kitsune_p2p::dependencies::kitsune_p2p_proxy::TlsConfig::new_ephemeral().await.unwrap())
                    .await?;

            let conductor = Conductor::new(
                envs.conductor(),
                envs.wasm(),
                self.dna_store,
                keystore,
                self.config.environment_path.clone(),
                holochain_p2p,
            )
            .await?;

            let conductor = Self::update_fake_state(self.state, conductor).await?;

            // Get data before handle
            let keystore = conductor.keystore.clone();
            let holochain_p2p = conductor.holochain_p2p.clone();

            // Create handle
            let handle: ConductorHandle = Arc::new(ConductorHandleImpl {
                root_env_dir: self.config.environment_path.clone(),
                conductor: RwLock::new(conductor),
                keystore,
                holochain_p2p,
                p2p_env: envs.p2p(),
                p2p_metrics_env: envs.p2p_metrics(),

                #[cfg(any(test, feature = "test_utils"))]
                dev_settings: parking_lot::RwLock::new(DevSettings::default()),
            });

            // Install extra DNAs, in particular:
            // the ones with InlineZomes will not be registered in the Wasm DB
            // and cannot be automatically loaded on conductor restart.
            for dna_file in extra_dnas {
                handle
                    .register_dna(dna_file.clone())
                    .await
                    .expect("Could not install DNA");
            }

            Self::finish(handle, self.config, p2p_evt).await
        }
    }
}

#[instrument(skip(p2p_evt, handle))]
async fn p2p_event_task(
    p2p_evt: holochain_p2p::event::HolochainP2pEventReceiver,
    handle: ConductorHandle,
) {
    /// The number of events we allow to run in parallel before
    /// starting to await on the join handles.
    const NUM_PARALLEL_EVTS: usize = 100;
    p2p_evt
        .for_each_concurrent(NUM_PARALLEL_EVTS, |evt| {
            let handle = handle.clone();
            async move {
                if let Err(e) = handle.dispatch_holochain_p2p_event(evt).await {
                    tracing::error!(
                        message = "error dispatching network event",
                        error = ?e,
                    );
                }
            }
            .in_current_span()
        })
        .await;

    tracing::warn!("p2p_event_task has ended");
}

#[cfg(test)]
pub mod tests;
