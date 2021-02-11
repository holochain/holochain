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
use super::api::RealAdminInterfaceApi;
use super::api::RealAppInterfaceApi;
use super::config::AdminInterfaceConfig;
use super::config::InterfaceDriver;
use super::dna_store::DnaDefBuf;
use super::dna_store::DnaStore;
use super::dna_store::RealDnaStore;
use super::entry_def_store::get_entry_defs;
use super::entry_def_store::EntryDefBuf;
use super::entry_def_store::EntryDefBufferKey;
use super::error::ConductorError;
use super::error::CreateAppError;
use super::handle::ConductorHandleImpl;
use super::interface::error::InterfaceResult;
use super::interface::websocket::spawn_admin_interface_task;
use super::interface::websocket::spawn_app_interface_task;
use super::interface::websocket::spawn_websocket_listener;
use super::interface::websocket::SIGNAL_BUFFER_SIZE;
use super::interface::SignalBroadcaster;
use super::manager::keep_alive_task;
use super::manager::spawn_task_manager;
use super::manager::ManagedTaskAdd;
use super::manager::ManagedTaskHandle;
use super::manager::TaskManagerRunHandle;
use super::p2p_store::all_agent_infos;
use super::p2p_store::get_single_agent_info;
use super::p2p_store::inject_agent_infos;
use super::paths::EnvironmentRootPath;
use super::state::AppInterfaceId;
use super::state::ConductorState;
use super::CellError;
use super::{api::CellConductorApi, state::AppInterfaceConfig};
use super::{api::CellConductorApiT, interface::AppInterfaceRuntime};
use crate::conductor::api::error::ConductorApiResult;
use crate::conductor::cell::Cell;
use crate::conductor::config::ConductorConfig;
use crate::conductor::dna_store::MockDnaStore;
use crate::conductor::error::ConductorResult;
use crate::conductor::handle::ConductorHandle;
use crate::core::queue_consumer::InitialQueueTriggers;
pub use builder::*;
use fallible_iterator::FallibleIterator;
use futures::future;
use futures::future::TryFutureExt;
use futures::stream::StreamExt;
use holo_hash::DnaHash;
use holochain_keystore::lair_keystore::spawn_lair_keystore;
use holochain_keystore::test_keystore::spawn_test_keystore;
use holochain_keystore::KeystoreSender;
use holochain_keystore::KeystoreSenderExt;
use holochain_lmdb::buffer::BufferedStore;
use holochain_lmdb::buffer::KvStore;
use holochain_lmdb::buffer::KvStoreT;
use holochain_lmdb::db;
use holochain_lmdb::env::EnvironmentKind;
use holochain_lmdb::env::EnvironmentWrite;
use holochain_lmdb::env::ReadManager;
use holochain_lmdb::exports::SingleStore;
use holochain_lmdb::fresh_reader;
use holochain_lmdb::prelude::*;
use holochain_state::source_chain::SourceChainBuf;
use holochain_state::wasm::WasmBuf;
use holochain_types::prelude::*;
use kitsune_p2p::agent_store::AgentInfoSigned;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tracing::*;

#[cfg(any(test, feature = "test_utils"))]
use super::handle::MockConductorHandleT;

/// Conductor-specific Cell state, this can probably be stored in a database.
/// Hypothesis: If nothing remains in this struct, then the Conductor state is
/// essentially immutable, and perhaps we just throw it out and make a new one
/// when we need to load new config, etc.
pub struct CellState {
    /// Whether or not we should call any methods on the cell
    _active: bool,
}

/// An [Cell] tracked by a Conductor, along with some [CellState]
struct CellItem<CA>
where
    CA: CellConductorApiT,
{
    cell: Arc<Cell<CA>>,
    _state: CellState,
}

pub type StopBroadcaster = tokio::sync::broadcast::Sender<()>;
pub type StopReceiver = tokio::sync::broadcast::Receiver<()>;

/// A Conductor is a group of [Cell]s
pub struct Conductor<DS = RealDnaStore, CA = CellConductorApi>
where
    DS: DnaStore,
    CA: CellConductorApiT,
{
    /// The collection of cells associated with this Conductor
    cells: HashMap<CellId, CellItem<CA>>,

    /// The LMDB environment for persisting state related to this Conductor
    env: EnvironmentWrite,

    /// An LMDB environment for storing wasm
    wasm_env: EnvironmentWrite,

    /// The LMDB environment for storing AgentInfoSigned
    p2p_env: EnvironmentWrite,

    /// The database for persisting [ConductorState]
    state_db: ConductorStateDb,

    /// Set to true when `conductor.shutdown()` has been called, so that other
    /// tasks can check on the shutdown status
    shutting_down: bool,

    /// The admin websocket ports this conductor has open.
    /// This exists so that we can run tests and bind to port 0, and find out
    /// the dynamically allocated port later.
    admin_websocket_ports: Vec<u16>,

    /// Collection app interface data, keyed by id
    app_interfaces: HashMap<AppInterfaceId, AppInterfaceRuntime>,

    /// Channel on which to send info about tasks we want to manage
    managed_task_add_sender: mpsc::Sender<ManagedTaskAdd>,

    /// By sending on this channel,
    managed_task_stop_broadcaster: StopBroadcaster,

    /// The main task join handle to await on.
    /// The conductor is intended to live as long as this task does.
    task_manager_run_handle: Option<TaskManagerRunHandle>,

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
        tracing::info!(
            "Sending shutdown signal to {} managed tasks.",
            self.managed_task_stop_broadcaster.receiver_count(),
        );
        self.managed_task_stop_broadcaster
            .send(())
            .map(|_| ())
            .unwrap_or_else(|e| {
                error!(?e, "Couldn't broadcast stop signal to managed tasks!");
            })
    }

    /// Return the handle which waits for the task manager task to complete
    pub(super) fn take_shutdown_handle(&mut self) -> Option<TaskManagerRunHandle> {
        self.task_manager_run_handle.take()
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
        let stop_tx = self.managed_task_stop_broadcaster.clone();

        // Closure to process each admin config item
        let spawn_from_config = |AdminInterfaceConfig { driver, .. }| {
            let admin_api = admin_api.clone();
            let stop_tx = stop_tx.clone();
            async move {
                match driver {
                    InterfaceDriver::Websocket { port } => {
                        let listener = spawn_websocket_listener(port).await?;
                        let port = listener.local_addr().port().unwrap_or(port);
                        let handle: ManagedTaskHandle = spawn_admin_interface_task(
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
            self.manage_task(ManagedTaskAdd::dont_handle(tokio::spawn(keep_alive_task(
                stop_tx.subscribe(),
            ))))
            .await?;

            // Now that tasks are spawned, register them with the TaskManager
            for (port, handle) in handles {
                ports.push(port);
                self.manage_task(ManagedTaskAdd::new(
                    handle,
                    Box::new(|result| {
                        result.unwrap_or_else(|e| {
                            error!(error = &e as &dyn std::error::Error, "Interface died")
                        });
                        None
                    }),
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
        port: u16,
        handle: ConductorHandle,
    ) -> ConductorResult<u16> {
        let interface_id: AppInterfaceId = format!("interface-{}", port).into();
        let app_api = RealAppInterfaceApi::new(handle, interface_id.clone());
        // This receiver is thrown away because we can produce infinite new
        // receivers from the Sender
        let (signal_tx, _r) = tokio::sync::broadcast::channel(SIGNAL_BUFFER_SIZE);
        let stop_rx = self.managed_task_stop_broadcaster.subscribe();
        let (port, task) = spawn_app_interface_task(port, app_api, signal_tx.clone(), stop_rx)
            .await
            .map_err(Box::new)?;
        // TODO: RELIABILITY: Handle this task by restarting it if it fails and log the error
        self.manage_task(ManagedTaskAdd::dont_handle(task)).await?;
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
        Ok(port)
    }

    /// Start all app interfaces currently in state.
    /// This should only be run at conductor initialization.
    #[allow(irrefutable_let_patterns)]
    pub(super) async fn startup_app_interfaces_via_handle(
        &mut self,
        handle: ConductorHandle,
    ) -> ConductorResult<()> {
        for i in self.get_state().await?.app_interfaces.values() {
            tracing::debug!("Starting up app interface: {:?}", i);
            let port = if let InterfaceDriver::Websocket { port } = i.driver {
                port
            } else {
                unreachable!()
            };
            let _ = self
                .add_app_interface_via_handle(port, handle.clone())
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

    /// Perform Genesis on the source chains for each of the specified CellIds.
    ///
    /// If genesis fails for any cell, this entire function fails, and all other
    /// partial or complete successes are rolled back.
    pub(super) async fn genesis_cells(
        &self,
        cell_ids_with_proofs: Vec<(CellId, Option<MembraneProof>)>,
        conductor_handle: ConductorHandle,
    ) -> ConductorResult<()> {
        let root_env_dir = std::path::PathBuf::from(self.root_env_dir.clone());
        let keystore = self.keystore.clone();

        let cells_tasks = cell_ids_with_proofs.into_iter().map(|(cell_id, proof)| {
            let root_env_dir = root_env_dir.clone();
            let keystore = self.keystore.clone();
            let conductor_handle = conductor_handle.clone();
            let cell_id_inner = cell_id.clone();
            tokio::spawn(async move {
                let env = EnvironmentWrite::new(
                    &root_env_dir,
                    EnvironmentKind::Cell(cell_id_inner.clone()),
                    keystore.clone(),
                )?;
                Cell::genesis(cell_id_inner, conductor_handle, env, proof).await
            })
            .map_err(CellError::from)
            .and_then(|result| async move { result.map(|_| cell_id) })
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
                let env = EnvironmentWrite::new(
                    &root_env_dir,
                    EnvironmentKind::Cell(cell_id),
                    keystore.clone(),
                )?;
                env.remove().await?;
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

    /// Create Cells for each CellId marked active in the ConductorState db
    pub(super) async fn create_active_app_cells(
        &self,
        conductor_handle: ConductorHandle,
    ) -> ConductorResult<Vec<Result<Vec<(Cell, InitialQueueTriggers)>, CreateAppError>>> {
        // Only create the active apps
        let active_apps = self.get_state().await?.active_apps;

        // Data required to create apps
        let root_env_dir = self.root_env_dir.clone();
        let keystore = self.keystore.clone();

        // Closure for creating all cells in an app
        let tasks = active_apps.into_iter().map(
            move |(installed_app_id, app): (InstalledAppId, InstalledApp)| {
                // Clone data for async block
                let root_env_dir = std::path::PathBuf::from(root_env_dir.clone());
                let conductor_handle = conductor_handle.clone();
                let keystore = keystore.clone();

                // Task that creates the cells
                async move {
                    // Only create cells not already created
                    let cells_to_create = app
                        .cells()
                        .filter(|cell_id| !self.cells.contains_key(cell_id))
                        .map(|cell_id| {
                            (
                                cell_id,
                                root_env_dir.clone(),
                                keystore.clone(),
                                conductor_handle.clone(),
                            )
                        });

                    use holochain_p2p::actor::HolochainP2pRefToCell;

                    // Create each cell
                    let cells_tasks = cells_to_create.map(
                        |(cell_id, dir, keystore, conductor_handle)| async move {
                            tracing::info!(?cell_id, "CREATE CELL");
                            let holochain_p2p_cell = self.holochain_p2p.to_cell(
                                cell_id.dna_hash().clone(),
                                cell_id.agent_pubkey().clone(),
                            );

                            let env = EnvironmentWrite::new_cell(
                                &dir,
                                cell_id.clone(),
                                keystore.clone(),
                            )?;
                            Cell::create(
                                cell_id.clone(),
                                conductor_handle.clone(),
                                env,
                                holochain_p2p_cell,
                                self.managed_task_add_sender.clone(),
                                self.managed_task_stop_broadcaster.clone(),
                            )
                            .await
                        },
                    );

                    // Join all the cell create tasks for this app
                    // and separate any errors
                    let (success, errors): (Vec<_>, Vec<_>) =
                        futures::future::join_all(cells_tasks)
                            .await
                            .into_iter()
                            .partition(Result::is_ok);
                    // unwrap safe because of the partition
                    let success = success.into_iter().map(Result::unwrap);

                    // If there was errors, cleanup and return the errors
                    if !errors.is_empty() {
                        for cell in success {
                            // Error needs to capture which app failed
                            cell.0.destroy().await.map_err(|e| CreateAppError::Failed {
                                installed_app_id: installed_app_id.clone(),
                                errors: vec![e],
                            })?;
                        }
                        // match needed to avoid Debug requirement on unwrap_err
                        let errors = errors
                            .into_iter()
                            .map(|e| match e {
                                Err(e) => e,
                                Ok(_) => unreachable!("Safe because of the partition"),
                            })
                            .collect();
                        Err(CreateAppError::Failed {
                            installed_app_id,
                            errors,
                        })
                    } else {
                        // No errors so return the cells
                        Ok(success.collect())
                    }
                }
            },
        );

        // Join on all apps and return a list of
        // apps that had succelly created cells
        // and any apps that encounted errors
        Ok(futures::future::join_all(tasks).await)
    }

    /// Register an app inactive in the database
    pub(super) async fn add_inactive_app_to_db(
        &mut self,
        app: InstalledApp,
    ) -> ConductorResult<()> {
        trace!(?app);
        self.update_state(move |mut state| {
            debug!(?app);
            let is_active = state.active_apps.contains_key(app.installed_app_id());
            let is_inactive = state.inactive_apps.insert(app.clone()).is_some();
            if is_active || is_inactive {
                Err(ConductorError::AppAlreadyInstalled(
                    app.installed_app_id().clone(),
                ))
            } else {
                Ok(state)
            }
        })
        .await?;
        Ok(())
    }

    /// Activate an app in the database
    pub(super) async fn activate_app_in_db(
        &mut self,
        installed_app_id: InstalledAppId,
    ) -> ConductorResult<()> {
        self.update_state(move |mut state| {
            let app = state
                .inactive_apps
                .remove(&installed_app_id)
                .ok_or_else(|| ConductorError::AppNotInstalled(installed_app_id.clone()))?;
            state.active_apps.insert(app);
            Ok(state)
        })
        .await?;
        Ok(())
    }

    /// Deactivate an app in the database
    pub(super) async fn deactivate_app_in_db(
        &mut self,
        installed_app_id: InstalledAppId,
    ) -> ConductorResult<Vec<CellId>> {
        let state = self
            .update_state({
                let installed_app_id = installed_app_id.clone();
                move |mut state| {
                    let app = state
                        .active_apps
                        .remove(&installed_app_id)
                        .ok_or_else(|| ConductorError::AppNotActive(installed_app_id.clone()))?;
                    state.inactive_apps.insert(app);
                    Ok(state)
                }
            })
            .await?;
        Ok(state
            .inactive_apps
            .get(&installed_app_id)
            .expect("This app was just put here")
            .clone()
            .cells()
            .cloned()
            .collect())
    }

    /// Add fully constructed cells to the cell map in the Conductor
    pub(super) fn add_cells(&mut self, cells: Vec<(Cell, InitialQueueTriggers)>) {
        for (cell, trigger) in cells {
            let cell_id = cell.id().clone();
            tracing::info!(?cell_id, "ADD CELL");
            self.cells.insert(
                cell_id,
                CellItem {
                    cell: Arc::new(cell),
                    _state: CellState { _active: false },
                },
            );

            trigger.initialize_workflows();
        }
    }

    /// Associate a Cell with an existing App
    pub(super) async fn add_clone_cell_to_app(
        &mut self,
        installed_app_id: &InstalledAppId,
        cell_nick: &CellNick,
        properties: YamlProperties,
    ) -> ConductorResult<CellId> {
        let (_, cell_id) = self
            .update_state_prime(|mut state| {
                if let Some(app) = state.active_apps.get_mut(installed_app_id) {
                    let cell_id = app.add_clone(cell_nick, properties)?;
                    Ok((state, cell_id))
                } else {
                    Err(ConductorError::AppNotActive(installed_app_id.clone()))
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
        let environ = &self.wasm_env;
        let wasm = environ.get_db(&*holochain_lmdb::db::WASM)?;
        let dna_def_db = environ.get_db(&*holochain_lmdb::db::DNA_DEF)?;
        let entry_def_db = environ.get_db(&*holochain_lmdb::db::ENTRY_DEF)?;

        let wasm_buf = Arc::new(WasmBuf::new(environ.clone().into(), wasm)?);
        let dna_def_buf = DnaDefBuf::new(environ.clone().into(), dna_def_db)?;
        let entry_def_buf = EntryDefBuf::new(environ.clone().into(), entry_def_db)?;
        // Load out all dna defs
        let wasm_tasks = dna_def_buf
            .get_all()?
            .into_iter()
            .map(|dna_def| {
                // Load all wasms for each dna_def from the wasm db into memory
                let wasms = dna_def.zomes.clone().into_iter().map(|(zome_name, zome)| {
                    let wasm_buf = wasm_buf.clone();
                    async move {
                        wasm_buf
                            .get(&zome.wasm_hash(&zome_name)?)
                            .await?
                            .map(|hashed| hashed.into_content())
                            .ok_or(ConductorError::WasmMissing)
                    }
                });
                async move {
                    let wasms = futures::future::try_join_all(wasms).await?;
                    let dna_file = DnaFile::new(dna_def.into_content(), wasms).await?;
                    ConductorResult::Ok((dna_file.dna_hash().clone(), dna_file))
                }
            })
            // This needs to happen due to the environment not being Send
            .collect::<Vec<_>>();
        // try to join all the tasks and return the list of dna files
        let dnas = futures::future::try_join_all(wasm_tasks).await?;
        let defs = fresh_reader!(environ, |r| entry_def_buf.get_all(&r)?.collect::<Vec<_>>())?;
        Ok((dnas, defs))
    }

    /// Remove cells from the cell map in the Conductor
    pub(super) fn remove_cells(&mut self, cell_ids: Vec<CellId>) {
        for cell_id in cell_ids {
            self.cells.remove(&cell_id);
        }
    }

    pub(super) fn add_agent_infos(
        &self,
        agent_infos: Vec<AgentInfoSigned>,
    ) -> ConductorApiResult<()> {
        Ok(inject_agent_infos(self.p2p_env.clone(), agent_infos)?)
    }

    pub(super) fn get_agent_infos(
        &self,
        cell_id: Option<CellId>,
    ) -> ConductorApiResult<Vec<AgentInfoSigned>> {
        match cell_id {
            Some(c) => {
                let (d, a) = c.into_dna_and_agent();
                Ok(get_single_agent_info(self.p2p_env.clone().into(), d, a)?
                    .map(|a| vec![a])
                    .unwrap_or_default())
            }
            None => Ok(all_agent_infos(self.p2p_env.clone().into())?),
        }
    }

    pub(super) async fn put_wasm(
        &self,
        dna: DnaFile,
    ) -> ConductorResult<Vec<(EntryDefBufferKey, EntryDef)>> {
        let environ = self.wasm_env.clone();
        let wasm = environ.get_db(&*holochain_lmdb::db::WASM)?;
        let dna_def_db = environ.get_db(&*holochain_lmdb::db::DNA_DEF)?;
        let entry_def_db = environ.get_db(&*holochain_lmdb::db::ENTRY_DEF)?;

        let zome_defs = get_entry_defs(dna.clone())?;

        let mut entry_def_buf = EntryDefBuf::new(environ.clone().into(), entry_def_db)?;

        for (key, entry_def) in zome_defs.clone() {
            entry_def_buf.put(key, entry_def)?;
        }

        let mut wasm_buf = WasmBuf::new(environ.clone().into(), wasm)?;
        let mut dna_def_buf = DnaDefBuf::new(environ.clone().into(), dna_def_db)?;
        // TODO: PERF: This loop might be slow
        for (wasm_hash, dna_wasm) in dna.code().clone().into_iter() {
            if wasm_buf.get(&wasm_hash).await?.is_none() {
                wasm_buf.put(DnaWasmHashed::from_content(dna_wasm).await);
            }
        }
        if dna_def_buf.get(dna.dna_hash()).await?.is_none() {
            dna_def_buf.put(dna.dna_def().clone()).await?;
        }
        {
            let env = environ.guard();
            // write the wasm db
            env.with_commit(|writer| wasm_buf.flush_to_txn(writer))?;

            // write the dna_def db
            env.with_commit(|writer| dna_def_buf.flush_to_txn(writer))?;

            // write the entry_def db
            env.with_commit(|writer| entry_def_buf.flush_to_txn(writer))?;
        }
        Ok(zome_defs)
    }

    pub(super) async fn list_cell_ids(&self) -> ConductorResult<Vec<CellId>> {
        Ok(self.cells.keys().cloned().collect())
    }

    pub(super) async fn list_active_apps(&self) -> ConductorResult<Vec<InstalledAppId>> {
        let active_apps = self.get_state().await?.active_apps;
        Ok(active_apps.keys().cloned().collect())
    }

    pub(super) async fn dump_cell_state(&self, cell_id: &CellId) -> ConductorApiResult<String> {
        let cell = self.cell_by_id(cell_id)?;
        let arc = cell.env();
        let source_chain = SourceChainBuf::new(arc.clone().into())?;
        Ok(source_chain.dump_as_json().await?)
    }

    pub(super) fn p2p_env(&self) -> EnvironmentWrite {
        self.p2p_env.clone()
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

//-----------------------------------------------------------------------------
// Private methods
//-----------------------------------------------------------------------------

impl<DS> Conductor<DS>
where
    DS: DnaStore + 'static,
{
    async fn new(
        env: EnvironmentWrite,
        wasm_env: EnvironmentWrite,
        p2p_env: EnvironmentWrite,
        dna_store: DS,
        keystore: KeystoreSender,
        root_env_dir: EnvironmentRootPath,
        holochain_p2p: holochain_p2p::HolochainP2pRef,
    ) -> ConductorResult<Self> {
        let db: SingleStore = env.get_db(&db::CONDUCTOR_STATE)?;
        let (task_tx, task_manager_run_handle) = spawn_task_manager();
        let task_manager_run_handle = Some(task_manager_run_handle);
        let (stop_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        Ok(Self {
            env,
            wasm_env,
            p2p_env,
            state_db: KvStore::new(db),
            cells: HashMap::new(),
            shutting_down: false,
            app_interfaces: HashMap::new(),
            managed_task_add_sender: task_tx,
            managed_task_stop_broadcaster: stop_tx,
            task_manager_run_handle,
            admin_websocket_ports: Vec::new(),
            dna_store,
            keystore,
            root_env_dir,
            holochain_p2p,
        })
    }

    pub(super) async fn get_state(&self) -> ConductorResult<ConductorState> {
        let guard = self.env.guard();
        let reader = guard.reader()?;
        Ok(self.state_db.get(&reader, &UnitDbKey)?.unwrap_or_default())
    }

    /// Update the internal state with a pure function mapping old state to new
    async fn update_state<F: Send>(&self, f: F) -> ConductorResult<ConductorState>
    where
        F: FnOnce(ConductorState) -> ConductorResult<ConductorState>,
    {
        let (state, _) = self.update_state_prime(|s| Ok((f(s)?, ()))).await?;
        Ok(state)
    }

    /// Update the internal state with a pure function mapping old state to new,
    /// which may also produce an output value which will be the output of
    /// this function
    async fn update_state_prime<F: Send, O>(&self, f: F) -> ConductorResult<(ConductorState, O)>
    where
        F: FnOnce(ConductorState) -> ConductorResult<(ConductorState, O)>,
    {
        self.check_running()?;
        let guard = self.env.guard();
        let output = guard.with_commit(|txn| {
            let state: ConductorState = self.state_db.get(txn, &UnitDbKey)?.unwrap_or_default();
            let (new_state, output) = f(state)?;
            self.state_db.put(txn, &UnitDbKey, &new_state)?;
            Result::<_, ConductorError>::Ok((new_state, output))
        })?;
        Ok(output)
    }

    fn add_admin_port(&mut self, port: u16) {
        self.admin_websocket_ports.push(port);
    }

    /// Sends a JoinHandle to the TaskManager task to be managed
    async fn manage_task(&mut self, handle: ManagedTaskAdd) -> ConductorResult<()> {
        self.managed_task_add_sender
            .send(handle)
            .await
            .map_err(|e| ConductorError::SubmitTaskError(format!("{}", e)))
    }
}

/// The database used to store ConductorState. It has only one key-value pair.
pub type ConductorStateDb = KvStore<UnitDbKey, ConductorState>;

mod builder {
    use super::*;
    use crate::conductor::dna_store::RealDnaStore;
    use crate::conductor::ConductorHandle;
    use holochain_lmdb::env::EnvironmentKind;
    #[cfg(any(test, feature = "test_utils"))]
    use holochain_lmdb::test_utils::TestEnvironments;

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
                spawn_lair_keystore(self.config.keystore_path.as_deref()).await?
            };
            let env_path = self.config.environment_path.clone();

            let environment = EnvironmentWrite::new(
                env_path.as_ref(),
                EnvironmentKind::Conductor,
                keystore.clone(),
            )?;

            let wasm_environment =
                EnvironmentWrite::new(env_path.as_ref(), EnvironmentKind::Wasm, keystore.clone())?;

            let p2p_environment =
                EnvironmentWrite::new(env_path.as_ref(), EnvironmentKind::P2p, keystore.clone())?;

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
                p2p_environment,
                dna_store,
                keystore,
                env_path,
                holochain_p2p,
            )
            .await?;

            #[cfg(any(test, feature = "test_utils"))]
            let conductor = Self::update_fake_state(state, conductor).await?;

            Self::finish(conductor, config, p2p_evt).await
        }

        async fn finish(
            conductor: Conductor<DS>,
            conductor_config: ConductorConfig,
            p2p_evt: holochain_p2p::event::HolochainP2pEventReceiver,
        ) -> ConductorResult<ConductorHandle> {
            // Get data before handle
            let keystore = conductor.keystore.clone();
            let holochain_p2p = conductor.holochain_p2p.clone();

            // Create handle
            let handle: ConductorHandle = Arc::new(ConductorHandleImpl {
                conductor: RwLock::new(conductor),
                keystore,
                holochain_p2p,
            });

            handle.load_dnas().await?;

            tokio::task::spawn(p2p_event_task(p2p_evt, handle.clone()));

            let cell_startup_errors = handle.clone().setup_cells().await?;

            // TODO: This should probably be emitted over the admin interface
            if !cell_startup_errors.is_empty() {
                error!(
                    msg = "Failed to create the following active apps",
                    ?cell_startup_errors
                );
            }

            // Create admin interfaces
            if let Some(configs) = conductor_config.admin_interfaces {
                handle.clone().add_admin_interfaces(configs).await?;
            }

            // Create app interfaces
            handle.clone().startup_app_interfaces().await?;

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
        pub async fn test(self, envs: &TestEnvironments) -> ConductorResult<ConductorHandle> {
            let keystore = envs.conductor().keystore();
            let (holochain_p2p, p2p_evt) =
                holochain_p2p::spawn_holochain_p2p(self.config.network.clone().unwrap_or_default(), holochain_p2p::kitsune_p2p::dependencies::kitsune_p2p_proxy::TlsConfig::new_ephemeral().await.unwrap())
                    .await?;
            let conductor = Conductor::new(
                envs.conductor(),
                envs.wasm(),
                envs.p2p(),
                self.dna_store,
                keystore,
                envs.tempdir().path().to_path_buf().into(),
                holochain_p2p,
            )
            .await?;

            #[cfg(any(test, feature = "test_utils"))]
            let conductor = Self::update_fake_state(self.state, conductor).await?;

            Self::finish(conductor, self.config, p2p_evt).await
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
                let cell_id =
                    CellId::new(evt.dna_hash().clone(), evt.target_agent_as_ref().clone());
                if let Err(e) = handle.dispatch_holochain_p2p_event(&cell_id, evt).await {
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
