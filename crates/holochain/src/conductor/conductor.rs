#![deny(missing_docs)]
//! A Conductor is a dynamically changing group of [Cell]s.
//!
//! A Conductor can be managed:
//! - externally, via an [`AppInterfaceApi`](super::api::AppInterfaceApi)
//! - from within a [`Cell`](super::Cell), via [`CellConductorApi`](super::api::CellConductorApi)
//!
//! In normal use cases, a single Holochain user runs a single Conductor in a single process.
//! However, there's no reason we can't have multiple Conductors in a single process, simulating multiple
//! users in a testing environment.

pub use self::share::RwShare;
use super::api::RealAppInterfaceApi;
use super::config::AdminInterfaceConfig;
use super::config::InterfaceDriver;
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
use super::paths::DatabaseRootPath;
use super::ribosome_store::RibosomeStore;
use super::space::Space;
use super::space::Spaces;
use super::state::AppInterfaceId;
use super::state::ConductorState;
use super::CellError;
use super::{api::CellConductorApi, state::AppInterfaceConfig};
use super::{api::CellConductorApiT, interface::AppInterfaceRuntime};
use super::{api::RealAdminInterfaceApi, manager::TaskManagerClient};
use crate::conductor::cell::Cell;
use crate::conductor::config::ConductorConfig;
use crate::conductor::error::ConductorResult;
use crate::conductor::handle::ConductorHandle;
use crate::core::queue_consumer::InitialQueueTriggers;
use crate::core::queue_consumer::QueueConsumerMap;
use crate::core::ribosome::guest_callback::post_commit::PostCommitArgs;
use crate::core::ribosome::guest_callback::post_commit::POST_COMMIT_CHANNEL_BOUND;
use crate::core::ribosome::guest_callback::post_commit::POST_COMMIT_CONCURRENT_LIMIT;
use crate::core::ribosome::RibosomeT;
use crate::{
    conductor::api::error::ConductorApiResult, core::ribosome::real_ribosome::RealRibosome,
};
pub use builder::*;
use futures::future;
use futures::future::TryFutureExt;
use futures::stream::StreamExt;
use holo_hash::DnaHash;
use holochain_conductor_api::conductor::KeystoreConfig;
use holochain_conductor_api::AppStatusFilter;
use holochain_conductor_api::FullIntegrationStateDump;
use holochain_conductor_api::InstalledAppInfo;
use holochain_conductor_api::IntegrationStateDump;
use holochain_keystore::lair_keystore::spawn_lair_keystore;
use holochain_keystore::test_keystore::spawn_test_keystore;
use holochain_keystore::MetaLairClient;
use holochain_sqlite::prelude::*;
use holochain_sqlite::sql::sql_cell::state_dump;
use holochain_state::prelude::from_blob;
use holochain_state::prelude::StateMutationResult;
use holochain_state::prelude::StateQueryResult;
use holochain_types::prelude::*;
pub use holochain_types::share;
use rusqlite::Transaction;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::mpsc::error::SendError;
use tracing::*;

#[cfg(feature = "test_utils")]
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

    /// The Cell is currently in the process of trying to join the network.
    Joining,
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
#[derive(Clone)]
pub struct Conductor<CA = CellConductorApi>
where
    CA: CellConductorApiT,
{
    /// The collection of cells associated with this Conductor
    cells: RwShare<HashMap<CellId, CellItem<CA>>>,

    /// The config used to create this Conductor
    pub config: ConductorConfig,

    /// The map of dna hash spaces.
    pub(super) spaces: Spaces,

    /// Set to true when `conductor.shutdown()` has been called, so that other
    /// tasks can check on the shutdown status
    shutting_down: Arc<AtomicBool>,

    /// The admin websocket ports this conductor has open.
    /// This exists so that we can run tests and bind to port 0, and find out
    /// the dynamically allocated port later.
    admin_websocket_ports: RwShare<Vec<u16>>,

    /// Collection app interface data, keyed by id
    app_interfaces: RwShare<HashMap<AppInterfaceId, AppInterfaceRuntime>>,

    /// The channels and handles needed to interact with the task_manager task.
    /// If this is None, then the task manager has not yet been initialized.
    pub(super) task_manager: RwShare<Option<TaskManagerClient>>,

    /// Placeholder for what will be the real DNA/Wasm cache
    ribosome_store: RwShare<RibosomeStore>,

    /// Access to private keys for signing and encryption.
    keystore: MetaLairClient,

    /// Handle to the network actor.
    holochain_p2p: holochain_p2p::HolochainP2pRef,

    post_commit: tokio::sync::mpsc::Sender<PostCommitArgs>,
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
impl Conductor {
    /// Returns a port which is guaranteed to have a websocket listener with an Admin interface
    /// on it. Useful for specifying port 0 and letting the OS choose a free port.
    pub fn get_arbitrary_admin_websocket_port(&self) -> Option<u16> {
        self.admin_websocket_ports.share_ref(|p| p.get(0).copied())
    }
}

//-----------------------------------------------------------------------------
/// Methods used by the [ConductorHandle]
//-----------------------------------------------------------------------------
impl Conductor {
    pub(super) fn cell_by_id(&self, cell_id: &CellId) -> ConductorResult<Arc<Cell>> {
        let cell = self
            .cells
            .share_ref(|c| c.get(cell_id).map(|i| i.cell.clone()))
            .ok_or_else(|| ConductorError::CellMissing(cell_id.clone()))?;
        Ok(cell)
    }

    /// Iterator over only the cells which are fully running. Generally used
    /// to handle conductor interface requests
    pub(super) fn running_cell_ids(&self) -> HashSet<CellId> {
        self.cells.share_ref(|c| {
            c.iter()
                .filter_map(|(id, item)| {
                    if item.is_running() {
                        Some(id.clone())
                    } else {
                        None
                    }
                })
                .collect()
        })
    }

    /// Return Cells which are pending network join, and mark them as
    /// currently joining.
    ///
    /// Used to discover which cells need to be joined to the network.
    /// The cells' status are upgraded to `Joining` when this function is called.
    pub(super) fn mark_pending_cells_as_joining(&self) -> Vec<(CellId, Arc<Cell>)> {
        self.cells.share_mut(|cells| {
            cells
                .iter_mut()
                .filter_map(|(id, item)| {
                    if item.is_pending() {
                        item.status = CellStatus::Joining;
                        Some((id.clone(), item.cell.clone()))
                    } else {
                        None
                    }
                })
                .collect()
        })
    }

    /// A gate to put at the top of public functions to ensure that work is not
    /// attempted after a shutdown has been issued
    pub(super) fn check_running(&self) -> ConductorResult<()> {
        if self
            .shutting_down
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            Err(ConductorError::ShuttingDown)
        } else {
            Ok(())
        }
    }

    pub(super) fn ribosome_store(&self) -> &RwShare<RibosomeStore> {
        &self.ribosome_store
    }

    /// Broadcasts the shutdown signal to all managed tasks.
    /// To actually wait for these tasks to complete, be sure to
    /// `take_shutdown_handle` to await for completion.
    pub(super) fn shutdown(&self) {
        self.shutting_down
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.task_manager.share_ref(|tm| {
            if let Some(manager) = tm {
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
        });
    }

    /// Return the handle which waits for the task manager task to complete
    pub(super) fn take_shutdown_handle(&self) -> Option<TaskManagerRunHandle> {
        self.task_manager
            .share_mut(|tm| tm.as_mut().and_then(|manager| manager.take_handle()))
    }

    /// Spawn all admin interface tasks, register them with the TaskManager,
    /// and modify the conductor accordingly, based on the config passed in
    pub(super) async fn add_admin_interfaces_via_handle(
        &self,
        configs: Vec<AdminInterfaceConfig>,
        handle: ConductorHandle,
    ) -> ConductorResult<()> {
        let admin_api = RealAdminInterfaceApi::new(handle);
        let stop_tx = self.task_manager.share_ref(|tm| {
            tm.as_ref()
                .expect("Task manager not started yet")
                .task_stop_broadcaster()
                .clone()
        });

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
        &self,
        port: either::Either<u16, AppInterfaceId>,
        handle: ConductorHandle,
    ) -> ConductorResult<u16> {
        let interface_id = match port {
            either::Either::Left(port) => AppInterfaceId::new(port),
            either::Either::Right(id) => id,
        };
        let port = interface_id.port();
        tracing::debug!("Attaching interface {}", port);
        let app_api = RealAppInterfaceApi::new(handle);
        // This receiver is thrown away because we can produce infinite new
        // receivers from the Sender
        let (signal_tx, _r) = tokio::sync::broadcast::channel(SIGNAL_BUFFER_SIZE);
        let stop_rx = self.task_manager.share_ref(|tm| {
            tm.as_ref()
                .expect("Task manager not initialized")
                .task_stop_broadcaster()
                .subscribe()
        });
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

        self.app_interfaces.share_mut(|app_interfaces| {
            if app_interfaces.contains_key(&interface_id) {
                return Err(ConductorError::AppInterfaceIdCollision(
                    interface_id.clone(),
                ));
            }

            app_interfaces.insert(interface_id.clone(), interface);
            Ok(())
        })?;
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
        ribosome: RealRibosome,
    ) -> ConductorResult<Vec<(EntryDefBufferKey, EntryDef)>> {
        let is_full_wasm_dna = ribosome
            .dna_def()
            .all_zomes()
            .all(|(_, zome_def)| matches!(zome_def, ZomeDef::Wasm(_)));

        // Only install wasm if the DNA is composed purely of WasmZomes (no InlineZomes)
        if is_full_wasm_dna {
            Ok(self.put_wasm(ribosome).await?)
        } else {
            Ok(Vec::with_capacity(0))
        }
    }

    pub(super) fn register_dna_entry_defs(&self, entry_defs: Vec<(EntryDefBufferKey, EntryDef)>) {
        self.ribosome_store
            .share_mut(|d| d.add_entry_defs(entry_defs));
    }

    pub(super) fn register_phenotype(&self, ribosome: RealRibosome) {
        self.ribosome_store.share_mut(|d| d.add_ribosome(ribosome));
    }

    pub(super) fn get_queue_consumer_workflows(&self) -> QueueConsumerMap {
        self.spaces.queue_consumer_map.clone()
    }

    /// Start all app interfaces currently in state.
    /// This should only be run at conductor initialization.
    #[allow(irrefutable_let_patterns)]
    pub(super) async fn startup_app_interfaces_via_handle(
        &self,
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
        let senders = self
            .app_interfaces
            .share_ref(|ai| ai.values().map(|i| i.signal_tx()).cloned().collect());
        SignalBroadcaster::new(senders)
    }

    /// Instantiate a Ribosome for use with a DNA
    pub(crate) fn get_ribosome(&self, dna_hash: &DnaHash) -> ConductorResult<RealRibosome> {
        self.ribosome_store
            .share_ref(|d| match d.get_ribosome(dna_hash) {
                Some(r) => Ok(r),
                None => Err(DnaError::DnaMissing(dna_hash.to_owned()).into()),
            })
    }

    /// Get a dna space or create it if one doesn't exist.
    pub(super) fn get_or_create_space(&self, dna_hash: &DnaHash) -> ConductorResult<Space> {
        self.spaces.get_or_create_space(dna_hash)
    }

    pub(super) fn get_or_create_authored_db(
        &self,
        dna_hash: &DnaHash,
    ) -> ConductorResult<DbWrite<DbKindAuthored>> {
        self.spaces.authored_db(dna_hash)
    }

    pub(super) fn get_or_create_dht_db(
        &self,
        dna_hash: &DnaHash,
    ) -> ConductorResult<DbWrite<DbKindDht>> {
        self.spaces.dht_db(dna_hash)
    }

    /// Adjust app statuses (via state transitions) to match the current
    /// reality of which Cells are present in the conductor.
    /// - Do not change state for Disabled apps. For all others:
    /// - If an app is Paused but all of its (required) Cells are on,
    ///     then set it to Running
    /// - If an app is Running but at least one of its (required) Cells are off,
    ///     then set it to Paused
    pub(super) async fn reconcile_app_status_with_cell_status<S>(
        &self,
        app_ids: Option<S>,
    ) -> ConductorResult<AppStatusFx>
    where
        S: Into<HashSet<InstalledAppId>>,
    {
        use AppStatus::*;
        use AppStatusTransition::*;

        let app_ids: Option<HashSet<InstalledAppId>> = app_ids.map(S::into);
        let running_cells: HashSet<CellId> = self.running_cell_ids();
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
    pub(super) async fn remove_dangling_cells(&self) -> ConductorResult<()> {
        let state = self.get_state().await?;
        let keepers: HashSet<CellId> = state
            .enabled_apps()
            .flat_map(|(_, app)| app.all_cells().cloned().collect::<HashSet<_>>())
            .collect();

        // Clean up all cells that will be dropped (leave network, etc.)
        let to_cleanup: Vec<_> = self.cells.share_mut(|cells| {
            let to_remove = cells
                .keys()
                .filter(|id| !keepers.contains(id))
                .cloned()
                .collect::<Vec<_>>();

            to_remove
                .iter()
                .filter_map(|cell_id| cells.remove(cell_id))
                .collect()
        });
        for cell in to_cleanup {
            cell.cell.cleanup().await?;
        }

        // drop all but the keepers
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
        let (managed_task_add_sender, managed_task_stop_broadcaster) =
            self.task_manager.share_ref(|tm| {
                let tm = tm.as_ref().expect("Task manager not initialized");
                (
                    tm.task_add_sender().clone(),
                    tm.task_stop_broadcaster().clone(),
                )
            });

        // Closure for creating all cells in an app
        let state = self.get_state().await?;

        // Collect all CellIds across all apps, deduped
        let app_cells: HashSet<CellId> = state
            .installed_apps()
            .iter()
            .filter(|(_, app)| app.status().is_running())
            .flat_map(|(_id, app)| app.all_cells().collect::<Vec<&CellId>>())
            .cloned()
            .collect();

        // calculate the existing cells so we can filter those out, only creating
        // cells for CellIds that don't have cells
        let on_cells: HashSet<CellId> = self.cells.share_ref(|c| c.keys().cloned().collect());

        let tasks = app_cells.difference(&on_cells).map(|cell_id| {
            let conductor_handle = conductor_handle.clone();
            let managed_task_add_sender = managed_task_add_sender.clone();
            let managed_task_stop_broadcaster = managed_task_stop_broadcaster.clone();
            async move {
                use holochain_p2p::actor::HolochainP2pRefToDna;
                let holochain_p2p_cell = self.holochain_p2p.to_dna(cell_id.dna_hash().clone());

                let space = self
                    .get_or_create_space(cell_id.dna_hash())
                    .map_err(|e| CellError::FailedToCreateDnaSpace(e.into()))
                    .map_err(|err| (cell_id.clone(), err))?;

                Cell::create(
                    cell_id.clone(),
                    conductor_handle,
                    space,
                    holochain_p2p_cell,
                    managed_task_add_sender,
                    managed_task_stop_broadcaster,
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
        &self,
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
        &self,
        app_id: InstalledAppId,
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
        &self,
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
    pub(super) fn add_and_initialize_cells(&self, cells: Vec<(Cell, InitialQueueTriggers)>) {
        let (new_cells, triggers): (Vec<_>, Vec<_>) = cells.into_iter().unzip();
        self.cells.share_mut(|cells| {
            for cell in new_cells {
                let cell_id = cell.id().clone();
                tracing::debug!(?cell_id, "added cell");
                cells.insert(
                    cell_id,
                    CellItem {
                        cell: Arc::new(cell),
                        status: CellStatus::PendingJoin,
                    },
                );
            }
        });
        for trigger in triggers {
            trigger.initialize_workflows();
        }
    }

    /// Change the CellStatus of the given Cells in the Conductor.
    /// Silently ignores Cells that don't exist.
    pub(super) fn update_cell_status(&self, cell_ids: &[CellId], status: CellStatus) {
        for cell_id in cell_ids {
            self.cells.share_mut(|cells| {
                if let Some(mut cell) = cells.get_mut(cell_id) {
                    cell.status = status.clone();
                }
            });
        }
    }

    /// Associate a Cell with an existing App
    pub(super) async fn add_clone_cell_to_app(
        &self,
        app_id: InstalledAppId,
        role_id: AppRoleId,
        properties: YamlProperties,
    ) -> ConductorResult<CellId> {
        let ribosome_store = &self.ribosome_store;
        let (_, parent_dna_hash) = self
            .update_state_prime({
                let app_id = app_id.clone();
                let role_id = role_id.clone();
                move |mut state| {
                    if let Some(app) = state.installed_apps_mut().get_mut(&app_id) {
                        let role = app
                            .roles()
                            .get(&role_id)
                            .ok_or_else(|| AppError::AppRoleIdMissing(role_id.to_owned()))?;
                        let parent_dna_hash = role.dna_hash().clone();
                        Ok((state, parent_dna_hash))
                    } else {
                        Err(ConductorError::AppNotRunning(app_id.clone()))
                    }
                }
            })
            .await?;
        let child_dna = ribosome_store.share_ref(|ds| {
            ds.get_dna_file(&parent_dna_hash)
                .ok_or(DnaError::DnaMissing(parent_dna_hash))?
                .modify_phenotype(random_network_seed(), properties)
        })?;
        let child_dna_hash = child_dna.dna_hash().to_owned();
        let child_ribosome = RealRibosome::new(child_dna)?;
        self.register_phenotype(child_ribosome);
        let (_, cell_id) = self
            .update_state_prime(move |mut state| {
                if let Some(app) = state.installed_apps_mut().get_mut(&app_id) {
                    let agent_key = app.role(&role_id)?.agent_key().to_owned();
                    let cell_id = CellId::new(child_dna_hash, agent_key);
                    app.add_clone(&role_id, cell_id.clone())?;
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
        impl IntoIterator<Item = (DnaHash, RealRibosome)>,
        impl IntoIterator<Item = (EntryDefBufferKey, EntryDef)>,
    )> {
        let db = &self.spaces.wasm_db;

        // Load out all dna defs
        let (wasm_tasks, defs) = db
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
                            .all_zomes()
                            .map(|(zome_name, zome)| Ok(zome.wasm_hash(zome_name)?))
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
                let wasm_tasks = holochain_state::dna_def::get_all(&txn)?
                    .into_iter()
                    .map(|dna_def| {
                        // Load all wasms for each dna_def from the wasm db into memory
                        let wasms = dna_def.all_zomes().filter_map(|(zome_name, zome)| {
                            let wasm_hash = zome.wasm_hash(zome_name).ok()?;
                            // Note this is a cheap arc clone.
                            wasms.get(&wasm_hash).cloned()
                        });
                        let wasms = wasms.collect::<Vec<_>>();
                        async move {
                            let dna_file = DnaFile::new(dna_def.into_content(), wasms).await?;
                            let ribosome = RealRibosome::new(dna_file)?;
                            ConductorResult::Ok((ribosome.dna_hash().clone(), ribosome))
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
    pub fn root_db_dir(&self) -> &DatabaseRootPath {
        &self.spaces.db_dir
    }

    /// Get the keystore.
    pub fn keystore(&self) -> &MetaLairClient {
        &self.keystore
    }

    /// Get a reference to the conductor's HolochainP2p.
    pub fn holochain_p2p(&self) -> &holochain_p2p::HolochainP2pRef {
        &self.holochain_p2p
    }

    /// Remove cells from the cell map in the Conductor
    pub(super) async fn remove_cells(&self, cell_ids: Vec<CellId>) {
        let to_cleanup: Vec<_> = self.cells.share_mut(|cells| {
            cell_ids
                .into_iter()
                .filter_map(|cell_id| cells.remove(&cell_id).map(|c| (cell_id, c)))
                .collect()
        });
        for (cell_id, item) in to_cleanup {
            if let Err(err) = item.cell.cleanup().await {
                tracing::error!("Error cleaning up Cell: {:?}\nCellId: {}", err, cell_id);
            }
        }
    }

    /// Restart every paused app
    pub(super) async fn start_paused_apps(&self) -> ConductorResult<AppStatusFx> {
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
        ribosome: RealRibosome,
    ) -> ConductorResult<Vec<(EntryDefBufferKey, EntryDef)>> {
        let dna_def = ribosome.dna_def().clone();
        let code = ribosome
            .dna_file()
            .code()
            .clone()
            .into_iter()
            .map(|(_, c)| c);
        let zome_defs = get_entry_defs(ribosome).await?;
        self.put_wasm_code(dna_def, code, zome_defs).await
    }

    pub(super) async fn put_wasm_code(
        &self,
        dna: DnaDefHashed,
        code: impl Iterator<Item = wasm::DnaWasm>,
        zome_defs: Vec<(EntryDefBufferKey, EntryDef)>,
    ) -> ConductorResult<Vec<(EntryDefBufferKey, EntryDef)>> {
        // TODO: PERF: This loop might be slow
        let wasms = futures::future::join_all(code.map(DnaWasmHashed::from_content)).await;

        self.spaces
            .wasm_db
            .async_commit({
                let zome_defs = zome_defs.clone();
                move |txn| {
                    for dna_wasm in wasms {
                        if !holochain_state::wasm::contains(txn, dna_wasm.as_hash())? {
                            holochain_state::wasm::put(txn, dna_wasm)?;
                        }
                    }

                    for (key, entry_def) in zome_defs.clone() {
                        holochain_state::entry_def::put(txn, key, &entry_def)?;
                    }

                    if !holochain_state::dna_def::contains(txn, dna.as_hash())? {
                        holochain_state::dna_def::put(txn, dna.into_content())?;
                    }
                    StateMutationResult::Ok(())
                }
            })
            .await?;

        Ok(zome_defs)
    }

    pub(super) fn list_cell_ids(&self, filter: Option<CellStatusFilter>) -> Vec<CellId> {
        self.cells.share_ref(|cells| {
            cells
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
                .collect()
        })
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

    pub(super) async fn find_cell_with_role_alongside_cell(
        &self,
        cell_id: &CellId,
        role_id: &AppRoleId,
    ) -> ConductorResult<Option<CellId>> {
        Ok(self
            .get_state()
            .await?
            .running_apps()
            .find(|(_, running_app)| running_app.all_cells().any(|i| i == cell_id))
            .and_then(|(_, running_app)| {
                running_app
                    .into_common()
                    .role(role_id)
                    .ok()
                    .map(|role| role.cell_id())
                    .cloned()
            }))
    }

    pub(super) async fn list_running_apps_for_dna_hash(
        &self,
        dna_hash: &DnaHash,
    ) -> ConductorResult<HashSet<InstalledAppId>> {
        Ok(self
            .get_state()
            .await?
            .running_apps()
            .filter(|(_, v)| v.all_cells().any(|i| i.dna_hash() == dna_hash))
            .map(|(k, _)| k)
            .cloned()
            .collect())
    }

    pub(super) fn print_setup(&self) {
        use std::fmt::Write;
        let mut out = String::new();
        self.admin_websocket_ports
            .share_ref(|admin_websocket_ports| {
                for port in admin_websocket_ports {
                    writeln!(&mut out, "###ADMIN_PORT:{}###", port)
                        .expect("Can't write setup to std out");
                }
            });
        println!("\n###HOLOCHAIN_SETUP###\n{}###HOLOCHAIN_SETUP_END###", out);
    }

    #[cfg(any(test, feature = "test_utils"))]
    pub(super) async fn get_state_from_handle(&self) -> ConductorResult<ConductorState> {
        self.get_state().await
    }

    #[cfg(any(test, feature = "test_utils"))]
    pub(super) async fn add_test_app_interface<I: Into<AppInterfaceId>>(
        &self,
        id: I,
    ) -> ConductorResult<()> {
        let id = id.into();
        let (signal_tx, _r) = tokio::sync::broadcast::channel(1000);
        self.app_interfaces.share_mut(|app_interfaces| {
            if app_interfaces.contains_key(&id) {
                return Err(ConductorError::AppInterfaceIdCollision(id));
            }
            let _ = app_interfaces.insert(id, AppInterfaceRuntime::Test { signal_tx });
            Ok(())
        })
    }

    /// Get the post commit sender.
    pub async fn post_commit_permit(
        &self,
    ) -> Result<tokio::sync::mpsc::OwnedPermit<PostCommitArgs>, SendError<()>> {
        self.post_commit.clone().reserve_owned().await
    }
}

/// Perform Genesis on the source chains for each of the specified CellIds.
///
/// If genesis fails for any cell, this entire function fails, and all other
/// partial or complete successes are rolled back.
/// Note this function takes read locks so should not be called from within a read lock.
pub(super) async fn genesis_cells(
    conductor: &Conductor,
    cell_ids_with_proofs: Vec<(CellId, Option<MembraneProof>)>,
    conductor_handle: ConductorHandle,
) -> ConductorResult<()> {
    let cells_tasks = cell_ids_with_proofs.into_iter().map(|(cell_id, proof)| {
        let space = conductor
            .get_or_create_space(cell_id.dna_hash())
            .map_err(|e| CellError::FailedToCreateDnaSpace(e.into()));
        async {
            let space = space?;
            let authored_db = space.authored_db;
            let dht_db = space.dht_db;
            let dht_db_cache = space.dht_query_cache;
            let conductor_handle = conductor_handle.clone();
            let cell_id_inner = cell_id.clone();
            let ribosome = conductor_handle
                .get_ribosome(cell_id.dna_hash())
                .map_err(Box::new)?;
            tokio::spawn(async move {
                Cell::genesis(
                    cell_id_inner,
                    conductor_handle,
                    authored_db,
                    dht_db,
                    dht_db_cache,
                    ribosome,
                    proof,
                )
                .await
            })
            .map_err(CellError::from)
            .and_then(|result| async move { result.map(|_| cell_id) })
            .await
        }
    });
    let (success, errors): (Vec<_>, Vec<_>) = futures::future::join_all(cells_tasks)
        .await
        .into_iter()
        .partition(Result::is_ok);

    // unwrap safe because of the partition
    // TODO: Reference count the databases created here and clean them up on error.
    let _success = success.into_iter().map(Result::unwrap);

    // If there were errors, cleanup and return the errors
    if !errors.is_empty() {
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
pub async fn integration_dump(
    vault: &DbRead<DbKindDht>,
) -> ConductorApiResult<IntegrationStateDump> {
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
                AND
                (validation_stage IS NULL OR validation_stage < 3)
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

/// Dump the full integration json state.
/// Careful! This will return a lot of data.
pub async fn full_integration_dump(
    vault: &DbRead<DbKindDht>,
    dht_ops_cursor: Option<u64>,
) -> ConductorApiResult<FullIntegrationStateDump> {
    vault
        .async_reader(move |txn| {
            let integrated =
                query_dht_ops_from_statement(&txn, state_dump::DHT_OPS_INTEGRATED, dht_ops_cursor)?;

            let validation_limbo = query_dht_ops_from_statement(
                &txn,
                state_dump::DHT_OPS_IN_VALIDATION_LIMBO,
                dht_ops_cursor,
            )?;

            let integration_limbo = query_dht_ops_from_statement(
                &txn,
                state_dump::DHT_OPS_IN_INTEGRATION_LIMBO,
                dht_ops_cursor,
            )?;

            let dht_ops_cursor = txn.query_row(state_dump::DHT_OPS_ROW_ID, [], |row| row.get(0))?;

            ConductorApiResult::Ok(FullIntegrationStateDump {
                validation_limbo,
                integration_limbo,
                integrated,
                dht_ops_cursor,
            })
        })
        .await
}

fn query_dht_ops_from_statement(
    txn: &Transaction,
    stmt_str: &str,
    dht_ops_cursor: Option<u64>,
) -> ConductorApiResult<Vec<DhtOp>> {
    let final_stmt_str = match dht_ops_cursor {
        Some(cursor) => format!("{} AND rowid > {}", stmt_str, cursor),
        None => stmt_str.into(),
    };

    let mut stmt = txn.prepare(final_stmt_str.as_str())?;

    let r: Vec<DhtOp> = stmt
        .query_and_then([], |row| {
            let action = from_blob::<SignedAction>(row.get("action_blob")?)?;
            let op_type: DhtOpType = row.get("dht_type")?;
            let entry = match action.0.entry_type().map(|et| et.visibility()) {
                Some(EntryVisibility::Public) => {
                    let entry: Option<Vec<u8>> = row.get("entry_blob")?;
                    match entry {
                        Some(entry) => Some(from_blob::<Entry>(entry)?),
                        None => None,
                    }
                }
                _ => None,
            };
            Ok(DhtOp::from_type(op_type, action, entry)?)
        })?
        .collect::<StateQueryResult<Vec<_>>>()?;
    Ok(r)
}

//-----------------------------------------------------------------------------
// Private methods
//-----------------------------------------------------------------------------

impl Conductor {
    #[allow(clippy::too_many_arguments)]
    async fn new(
        config: ConductorConfig,
        ribosome_store: RwShare<RibosomeStore>,
        keystore: MetaLairClient,
        holochain_p2p: holochain_p2p::HolochainP2pRef,
        spaces: Spaces,
        post_commit: tokio::sync::mpsc::Sender<PostCommitArgs>,
    ) -> ConductorResult<Self> {
        Ok(Self {
            spaces,
            cells: RwShare::new(HashMap::new()),
            config,
            shutting_down: Arc::new(AtomicBool::new(false)),
            app_interfaces: RwShare::new(HashMap::new()),
            task_manager: RwShare::new(None),
            admin_websocket_ports: RwShare::new(Vec::new()),
            ribosome_store,
            keystore,
            holochain_p2p,
            post_commit,
        })
    }

    pub(crate) async fn prune_p2p_agents_db(&self) -> ConductorResult<()> {
        use holochain_p2p::AgentPubKeyExt;

        let mut space_to_agents = HashMap::new();

        for cell in self.cells.share_ref(|c| {
            <Result<_, one_err::OneErr>>::Ok(c.keys().cloned().collect::<Vec<_>>())
        })? {
            space_to_agents
                .entry(cell.dna_hash().clone())
                .or_insert_with(Vec::new)
                .push(cell.agent_pubkey().to_kitsune());
        }

        for (space, agents) in space_to_agents {
            let db = self.spaces.p2p_agents_db(&space)?;
            p2p_prune(&db, agents).await?;
        }

        Ok(())
    }

    pub(super) async fn get_state(&self) -> ConductorResult<ConductorState> {
        self.spaces.get_state().await
    }

    /// Update the internal state with a pure function mapping old state to new
    async fn update_state<F: Send>(&self, f: F) -> ConductorResult<ConductorState>
    where
        F: FnOnce(ConductorState) -> ConductorResult<ConductorState> + 'static,
    {
        self.spaces.update_state(f).await
    }

    /// Update the internal state with a pure function mapping old state to new,
    /// which may also produce an output value which will be the output of
    /// this function
    async fn update_state_prime<F, O>(&self, f: F) -> ConductorResult<(ConductorState, O)>
    where
        F: FnOnce(ConductorState) -> ConductorResult<(ConductorState, O)> + Send + 'static,
        O: Send + 'static,
    {
        self.check_running()?;
        self.spaces.update_state_prime(f).await
    }

    fn add_admin_port(&self, port: u16) {
        self.admin_websocket_ports.share_mut(|p| p.push(port));
    }

    /// Sends a JoinHandle to the TaskManager task to be managed
    async fn manage_task(&self, handle: ManagedTaskAdd) -> ConductorResult<()> {
        self.task_manager
            .share_ref(|tm| {
                tm.as_ref()
                    .expect("Task manager not initialized")
                    .task_add_sender()
                    .clone()
            })
            .send(handle)
            .await
            .map_err(|e| ConductorError::SubmitTaskError(format!("{}", e)))
    }
}

mod builder {
    use holochain_p2p::dht::ArqStrat;

    use super::*;
    use crate::conductor::handle::DevSettings;
    use crate::conductor::kitsune_host_impl::KitsuneHostImpl;
    use crate::conductor::ribosome_store::RibosomeStore;
    use crate::conductor::ConductorHandle;

    /// A configurable Builder for Conductor and sometimes ConductorHandle
    #[derive(Default)]
    pub struct ConductorBuilder {
        /// The configuration
        pub config: ConductorConfig,
        /// The RibosomeStore (mockable)
        pub ribosome_store: RibosomeStore,
        /// For new lair, passphrase is required
        pub passphrase: Option<sodoken::BufRead>,
        /// Optional keystore override
        pub keystore: Option<MetaLairClient>,
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

    impl ConductorBuilder {
        /// Set the ConductorConfig used to build this Conductor
        pub fn config(mut self, config: ConductorConfig) -> Self {
            self.config = config;
            self
        }

        /// Set the passphrase for use in keystore initialization
        pub fn passphrase(mut self, passphrase: Option<sodoken::BufRead>) -> Self {
            self.passphrase = passphrase;
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
            } else {
                match &self.config.keystore {
                    KeystoreConfig::DangerTestKeystore => spawn_test_keystore().await?,
                    KeystoreConfig::LairServer { connection_url } => {
                        let passphrase = match self.passphrase {
                            None => {
                                return Err(one_err::OneErr::new(
                                    "passphrase required for new lair keystore api",
                                )
                                .into())
                            }
                            Some(p) => p,
                        };
                        spawn_lair_keystore(connection_url.clone(), passphrase).await?
                    }
                    oth => unimplemented!("unimplemented keystore config: {:?}", oth),
                }
            };

            let Self {
                ribosome_store,
                config,
                ..
            } = self;

            let ribosome_store = RwShare::new(ribosome_store);

            let spaces = Spaces::new(&config)?;
            let tag = spaces.get_state().await?.tag().clone();

            let network_config = config.network.clone().unwrap_or_default();
            let (cert_digest, cert, cert_priv_key) =
                keystore.get_or_create_tls_cert_by_tag(tag.0).await?;
            let tls_config =
                holochain_p2p::kitsune_p2p::dependencies::kitsune_p2p_types::tls::TlsConfig {
                    cert,
                    cert_priv_key,
                    cert_digest,
                };
            let strat =
                ArqStrat::from_params(network_config.tuning_params.gossip_redundancy_target);

            let host = KitsuneHostImpl::new(
                spaces.clone(),
                ribosome_store.clone(),
                network_config.tuning_params.clone(),
                strat,
            );

            let (holochain_p2p, p2p_evt) =
                holochain_p2p::spawn_holochain_p2p(network_config, tls_config, host).await?;

            let (post_commit_sender, post_commit_receiver) =
                tokio::sync::mpsc::channel(POST_COMMIT_CHANNEL_BOUND);

            let conductor = Conductor::new(
                config.clone(),
                ribosome_store,
                keystore,
                holochain_p2p,
                spaces,
                post_commit_sender,
            )
            .await?;

            let shutting_down = conductor.shutting_down.clone();

            #[cfg(any(test, feature = "test_utils"))]
            let conductor = Self::update_fake_state(self.state, conductor).await?;

            // Create handle
            let handle: ConductorHandle = Arc::new(ConductorHandleImpl {
                conductor,

                #[cfg(any(test, feature = "test_utils"))]
                dev_settings: parking_lot::RwLock::new(DevSettings::default()),
            });

            {
                let handle = handle.clone();
                tokio::task::spawn(async move {
                    while !shutting_down.load(std::sync::atomic::Ordering::Relaxed) {
                        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                        if let Err(e) = handle.prune_p2p_agents_db().await {
                            tracing::error!("failed to prune p2p_agents_db: {:?}", e);
                        }
                    }
                });
            }

            Self::finish(handle, config, p2p_evt, post_commit_receiver).await
        }

        fn spawn_post_commit(
            conductor_handle: ConductorHandle,
            receiver: tokio::sync::mpsc::Receiver<PostCommitArgs>,
        ) {
            let receiver_stream = tokio_stream::wrappers::ReceiverStream::new(receiver);
            tokio::task::spawn(receiver_stream.for_each_concurrent(
                POST_COMMIT_CONCURRENT_LIMIT,
                move |post_commit_args| {
                    let conductor_handle = conductor_handle.clone();
                    async move {
                        let PostCommitArgs {
                            host_access,
                            invocation,
                            cell_id,
                        } = post_commit_args;
                        match conductor_handle.clone().get_ribosome(cell_id.dna_hash()) {
                            Ok(ribosome) => {
                                if let Err(e) = tokio::task::spawn_blocking(move || {
                                    if let Err(e) =
                                        ribosome.run_post_commit(host_access, invocation)
                                    {
                                        tracing::error!(?e);
                                    }
                                })
                                .await
                                {
                                    tracing::error!(?e);
                                }
                            }
                            Err(e) => {
                                tracing::error!(?e);
                            }
                        }
                    }
                },
            ));
        }

        async fn finish(
            handle: ConductorHandle,
            conductor_config: ConductorConfig,
            p2p_evt: holochain_p2p::event::HolochainP2pEventReceiver,
            post_commit_receiver: tokio::sync::mpsc::Receiver<PostCommitArgs>,
        ) -> ConductorResult<ConductorHandle> {
            tokio::task::spawn(p2p_event_task(p2p_evt, handle.clone()));

            let _ = handle
                .clone()
                .start_scheduler(holochain_zome_types::schedule::SCHEDULER_INTERVAL);

            Self::spawn_post_commit(handle.clone(), post_commit_receiver);

            let configs = conductor_config.admin_interfaces.unwrap_or_default();
            let cell_startup_errors = handle.clone().initialize_conductor(configs).await?;

            // TODO: This should probably be emitted over the admin interface
            if !cell_startup_errors.is_empty() {
                error!(
                    msg = "Failed to create the following active apps",
                    ?cell_startup_errors
                );
            }

            handle.print_setup();

            Ok(handle)
        }

        /// Pass a test keystore in, to ensure that generated test agents
        /// are actually available for signing (especially for tryorama compat)
        pub fn with_keystore(mut self, keystore: MetaLairClient) -> Self {
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
            conductor: Conductor,
        ) -> ConductorResult<Conductor> {
            if let Some(state) = state {
                conductor.update_state(move |_| Ok(state)).await?;
            }
            Ok(conductor)
        }

        /// Build a Conductor with a test environment
        #[cfg(any(test, feature = "test_utils"))]
        pub async fn test(
            mut self,
            env_path: &std::path::Path,
            extra_dnas: &[DnaFile],
        ) -> ConductorResult<ConductorHandle> {
            let keystore = self.keystore.unwrap_or_else(test_keystore);
            self.config.environment_path = env_path.to_path_buf().into();

            let spaces = Spaces::new(&self.config)?;

            let network_config = self.config.network.clone().unwrap_or_default();
            let tuning_params = network_config.tuning_params.clone();
            let strat = ArqStrat::from_params(tuning_params.gossip_redundancy_target);

            let ribosome_store = RwShare::new(self.ribosome_store);
            let host =
                KitsuneHostImpl::new(spaces.clone(), ribosome_store.clone(), tuning_params, strat);

            let (holochain_p2p, p2p_evt) =
                holochain_p2p::spawn_holochain_p2p(network_config, holochain_p2p::kitsune_p2p::dependencies::kitsune_p2p_types::tls::TlsConfig::new_ephemeral().await.unwrap(), host)
                    .await?;

            let (post_commit_sender, post_commit_receiver) =
                tokio::sync::mpsc::channel(POST_COMMIT_CHANNEL_BOUND);

            let conductor = Conductor::new(
                self.config.clone(),
                ribosome_store,
                keystore,
                holochain_p2p,
                spaces,
                post_commit_sender,
            )
            .await?;

            let conductor = Self::update_fake_state(self.state, conductor).await?;

            // Create handle
            let handle: ConductorHandle = Arc::new(ConductorHandleImpl {
                conductor,

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

            Self::finish(handle, self.config, p2p_evt, post_commit_receiver).await
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
    let num_tasks = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let max_time = Arc::new(std::sync::atomic::AtomicU64::new(0));
    p2p_evt
        .for_each_concurrent(NUM_PARALLEL_EVTS, |evt| {
            let handle = handle.clone();
            let num_tasks = num_tasks.clone();
            let max_time = max_time.clone();
            async move {
                let start = (num_tasks.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
                    >= NUM_PARALLEL_EVTS)
                    .then(std::time::Instant::now);

                if let Err(e) = handle.dispatch_holochain_p2p_event(evt).await {
                    tracing::error!(
                        message = "error dispatching network event",
                        error = ?e,
                    );
                }
                match start {
                    Some(start) => {
                        let el = start.elapsed();
                        let us = el.as_micros() as u64;
                        let max_us = max_time
                            .fetch_max(us, std::sync::atomic::Ordering::Relaxed)
                            .max(us);

                        let s = tracing::info_span!("holochain_perf", this_event_time = ?el, max_event_micros = %max_us);
                        s.in_scope(|| tracing::info!("dispatch_holochain_p2p_event is saturated"))
                    }
                    None => max_time.store(0, std::sync::atomic::Ordering::Relaxed),
                }
                num_tasks.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            }
            .in_current_span()
        })
        .await;

    tracing::warn!("p2p_event_task has ended");
}

#[cfg(test)]
pub mod tests;
