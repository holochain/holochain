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
use super::{
    api::{CellConductorApi, CellConductorApiT, RealAdminInterfaceApi, RealAppInterfaceApi},
    config::{AdminInterfaceConfig, InterfaceDriver},
    dna_store::{DnaDefBuf, DnaStore, RealDnaStore},
    entry_def_store::{get_entry_defs, EntryDefBuf, EntryDefBufferKey},
    error::{ConductorError, CreateAppError},
    handle::ConductorHandleImpl,
    interface::{
        error::InterfaceResult,
        websocket::{
            spawn_admin_interface_task, spawn_app_interface_task, spawn_websocket_listener,
            SIGNAL_BUFFER_SIZE,
        },
    },
    manager::{
        keep_alive_task, spawn_task_manager, ManagedTaskAdd, ManagedTaskHandle,
        TaskManagerRunHandle,
    },
    paths::EnvironmentRootPath,
    state::ConductorState,
    CellError,
};
use crate::{
    conductor::{
        api::error::ConductorApiResult, cell::Cell, config::ConductorConfig,
        dna_store::MockDnaStore, error::ConductorResult, handle::ConductorHandle,
    },
    core::state::{source_chain::SourceChainBuf, wasm::WasmBuf},
};
use holochain_keystore::{
    test_keystore::{spawn_test_keystore, MockKeypair},
    KeystoreApiSender, KeystoreSender,
};
use holochain_state::{
    buffer::BufferedStore,
    buffer::{KvStore, KvStoreT},
    db,
    env::{EnvironmentKind, EnvironmentWrite, ReadManager},
    exports::SingleStore,
    fresh_reader,
    prelude::*,
};
use holochain_types::{
    app::{AppId, InstalledApp, InstalledCell, MembraneProof},
    cell::CellId,
    dna::{wasm::DnaWasmHashed, DnaFile},
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::*;

pub use builder::*;
use futures::future::{self, TryFutureExt};
use holo_hash::DnaHash;

#[cfg(test)]
use super::handle::mock::MockConductorHandle;
use fallible_iterator::FallibleIterator;
use holochain_zome_types::entry_def::EntryDef;

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
    cell: Cell<CA>,
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

    /// The database for persisting [ConductorState]
    state_db: ConductorStateDb,

    /// Set to true when `conductor.shutdown()` has been called, so that other
    /// tasks can check on the shutdown status
    shutting_down: bool,

    /// The admin websocket ports this conductor has open.
    /// This exists so that we can run tests and bind to port 0, and find out
    /// the dynamically allocated port later.
    admin_websocket_ports: Vec<u16>,

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
    pub(super) fn cell_by_id(&self, cell_id: &CellId) -> ConductorResult<&Cell> {
        let item = self
            .cells
            .get(cell_id)
            .ok_or_else(|| ConductorError::CellMissing(cell_id.clone()))?;
        Ok(&item.cell)
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

    pub(super) fn shutdown(&mut self) {
        self.shutting_down = true;
        self.managed_task_stop_broadcaster
            .send(())
            .map(|_| ())
            .unwrap_or_else(|e| {
                error!(?e, "Couldn't broadcast stop signal to managed tasks!");
            })
    }

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
        let handles = handles.map_err(|e| Box::new(e))?;

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
        let app_api = RealAppInterfaceApi::new(handle);
        let (signal_broadcaster, _r) = tokio::sync::broadcast::channel(SIGNAL_BUFFER_SIZE);
        let stop_rx = self.managed_task_stop_broadcaster.subscribe();
        let (port, task) = spawn_app_interface_task(port, app_api, signal_broadcaster, stop_rx)
            .await
            .map_err(Box::new)?;
        // TODO: RELIABILITY: Handle this task by restating it if it fails and log the error
        self.manage_task(ManagedTaskAdd::dont_handle(task)).await?;
        Ok(port)
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
            let env = EnvironmentWrite::new(
                &root_env_dir,
                EnvironmentKind::Cell(cell_id.clone()),
                keystore.clone(),
            )
            .unwrap();
            tokio::spawn(Cell::genesis(
                cell_id.clone(),
                conductor_handle.clone(),
                env,
                proof,
            ))
            .map_err(|e| CellError::from(e))
            .and_then(|result| async { result.map(|_| cell_id) })
        });
        let (success, errors): (Vec<_>, Vec<_>) = futures::future::join_all(cells_tasks)
            .await
            .into_iter()
            .partition(Result::is_ok);

        // unwrap safe because of the partition
        let success = success.into_iter().map(Result::unwrap);

        // If there was errors, cleanup and return the errors
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
    ) -> ConductorResult<Vec<Result<Vec<Cell>, CreateAppError>>> {
        // Only create the active apps
        let active_apps = self.get_state().await?.active_apps;

        // Data required to create apps
        let root_env_dir = self.root_env_dir.clone();
        let keystore = self.keystore.clone();

        // Closure for creating all cells in an app
        let tasks =
            active_apps
                .into_iter()
                .map(move |(app_id, cells): (AppId, Vec<InstalledCell>)| {
                    let cell_ids = cells.into_iter().map(|c| c.into_id());
                    // Clone data for async block
                    let root_env_dir = std::path::PathBuf::from(root_env_dir.clone());
                    let conductor_handle = conductor_handle.clone();
                    let keystore = keystore.clone();

                    // Task that creates the cells
                    async move {
                        // Only create cells not already created
                        let cells_to_create = cell_ids
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
                                let holochain_p2p_cell = self.holochain_p2p.to_cell(
                                    cell_id.dna_hash().clone(),
                                    cell_id.agent_pubkey().clone(),
                                );

                                let env = EnvironmentWrite::new_cell(
                                    &dir,
                                    cell_id.clone(),
                                    keystore.clone(),
                                )?;
                                // .and_then(|env| {
                                Cell::create(
                                    cell_id.clone(),
                                    conductor_handle.clone(),
                                    env,
                                    holochain_p2p_cell,
                                    self.managed_task_add_sender.clone(),
                                    self.managed_task_stop_broadcaster.clone(),
                                )
                                .await
                                // })
                            },
                        );

                        // Join all the cell create tasks for this app
                        // and seperate any errors
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
                                cell.destroy().await.map_err(|e| CreateAppError::Failed {
                                    app_id: app_id.clone(),
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
                            Err(CreateAppError::Failed { app_id, errors })
                        } else {
                            // No errors so return the cells
                            Ok(success.collect())
                        }
                    }
                });

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
            state.inactive_apps.insert(app.app_id, app.cell_data);
            Ok(state)
        })
        .await?;
        Ok(())
    }

    /// Activate an app in the database
    pub(super) async fn activate_app_in_db(&mut self, app_id: AppId) -> ConductorResult<()> {
        self.update_state(move |mut state| {
            let cell_data = state
                .inactive_apps
                .remove(&app_id)
                .ok_or(ConductorError::AppNotInstalled)?;
            state.active_apps.insert(app_id, cell_data);
            Ok(state)
        })
        .await?;
        Ok(())
    }

    /// Deactivate an app in the database
    pub(super) async fn deactivate_app_in_db(
        &mut self,
        app_id: AppId,
    ) -> ConductorResult<Vec<CellId>> {
        let state = self
            .update_state({
                let app_id = app_id.clone();
                move |mut state| {
                    let cell_ids = state
                        .active_apps
                        .remove(&app_id)
                        .ok_or(ConductorError::AppNotActive)?;
                    state.inactive_apps.insert(app_id, cell_ids);
                    Ok(state)
                }
            })
            .await?;
        Ok(state
            .inactive_apps
            .get(&app_id)
            .expect("This app was just put here")
            .clone()
            .into_iter()
            .map(|c| c.into_id())
            .collect())
    }

    /// Add fully constructed cells to the cell map in the Conductor
    pub(super) fn add_cells(&mut self, cells: Vec<Cell>) {
        for cell in cells {
            self.cells.insert(
                cell.id().clone(),
                CellItem {
                    cell,
                    _state: CellState { _active: false },
                },
            );
        }
    }

    pub(super) async fn load_wasms_into_dna_files(
        &self,
    ) -> ConductorResult<(
        impl IntoIterator<Item = (DnaHash, DnaFile)>,
        impl IntoIterator<Item = (EntryDefBufferKey, EntryDef)>,
    )> {
        let environ = &self.wasm_env;
        let wasm = environ.get_db(&*holochain_state::db::WASM)?;
        let dna_def_db = environ.get_db(&*holochain_state::db::DNA_DEF)?;
        let entry_def_db = environ.get_db(&*holochain_state::db::ENTRY_DEF)?;

        let wasm_buf = Arc::new(WasmBuf::new(environ.clone().into(), wasm)?);
        let dna_def_buf = DnaDefBuf::new(environ.clone().into(), dna_def_db)?;
        let entry_def_buf = EntryDefBuf::new(environ.clone().into(), entry_def_db)?;
        // Load out all dna defs
        let wasm_tasks = dna_def_buf
            .get_all()
            .await?
            .into_iter()
            .map(|dna_def| {
                // Load all wasms for each dna_def from the wasm db into memory
                let wasms = dna_def.zomes.clone().into_iter().map(|(_, zome)| {
                    let wasm_buf = wasm_buf.clone();
                    async move {
                        wasm_buf
                            .get(&zome.wasm_hash)
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

    pub(super) async fn put_wasm(
        &self,
        dna: DnaFile,
    ) -> ConductorResult<Vec<(EntryDefBufferKey, EntryDef)>> {
        let environ = self.wasm_env.clone();
        let wasm = environ.get_db(&*holochain_state::db::WASM)?;
        let dna_def_db = environ.get_db(&*holochain_state::db::DNA_DEF)?;
        let entry_def_db = environ.get_db(&*holochain_state::db::ENTRY_DEF)?;

        let zome_defs = get_entry_defs(dna.clone()).await?;

        let mut entry_def_buf = EntryDefBuf::new(environ.clone().into(), entry_def_db)?;

        for (key, entry_def) in zome_defs.clone() {
            entry_def_buf.put(key, entry_def)?;
        }

        let mut wasm_buf = WasmBuf::new(environ.clone().into(), wasm)?;
        let mut dna_def_buf = DnaDefBuf::new(environ.clone().into(), dna_def_db)?;
        // TODO: PERF: This loop might be slow
        for (wasm_hash, dna_wasm) in dna.code().clone().into_iter() {
            if let None = wasm_buf.get(&wasm_hash).await? {
                wasm_buf.put(DnaWasmHashed::from_content(dna_wasm).await);
            }
        }
        if let None = dna_def_buf.get(dna.dna_hash()).await? {
            dna_def_buf.put(dna.dna().clone()).await?;
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

    pub(super) async fn dump_cell_state(&self, cell_id: &CellId) -> ConductorApiResult<String> {
        let cell = self.cell_by_id(cell_id)?;
        let arc = cell.env();
        let source_chain = SourceChainBuf::new(arc.clone().into())?;
        drop(arc);
        Ok(source_chain.dump_as_json().await?)
    }

    #[cfg(test)]
    pub(super) async fn get_state_from_handle(&self) -> ConductorResult<ConductorState> {
        self.get_state().await
    }
}

// -- TODO - delete this helper when we have a real keystore -- //

pub(crate) async fn delete_me_create_test_keystore() -> KeystoreSender {
    use std::convert::TryFrom;
    let keystore = spawn_test_keystore(vec![
        MockKeypair {
            pub_key: holo_hash::AgentPubKey::try_from(
                "uhCAkw-zrttiYpdfAYX4fR6W8DPUdheZJ-1QsRA4cTImmzTYUcOr4",
            )
            .unwrap(),
            sec_key: vec![
                220, 218, 15, 212, 178, 51, 204, 96, 121, 97, 6, 205, 179, 84, 80, 159, 84, 163,
                193, 46, 127, 15, 47, 91, 134, 106, 72, 72, 51, 76, 26, 16, 195, 236, 235, 182,
                216, 152, 165, 215, 192, 97, 126, 31, 71, 165, 188, 12, 245, 29, 133, 230, 73, 251,
                84, 44, 68, 14, 28, 76, 137, 166, 205, 54,
            ],
        },
        MockKeypair {
            pub_key: holo_hash::AgentPubKey::try_from(
                "uhCAkomHzekU0-x7p62WmrusdxD2w9wcjdajC88688JGSTEo6cbEK",
            )
            .unwrap(),
            sec_key: vec![
                170, 205, 134, 46, 233, 225, 100, 162, 101, 124, 207, 157, 12, 131, 239, 244, 216,
                190, 244, 161, 209, 56, 159, 135, 240, 134, 88, 28, 48, 75, 227, 244, 162, 97, 243,
                122, 69, 52, 251, 30, 233, 235, 101, 166, 174, 235, 29, 196, 61, 176, 247, 7, 35,
                117, 168, 194, 243, 206, 188, 240, 145, 146, 76, 74,
            ],
        },
    ])
    .await
    .unwrap();

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
}

// -- TODO - end -- //

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
            state_db: KvStore::new(db),
            cells: HashMap::new(),
            shutting_down: false,
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

    async fn update_state<F: Send>(&self, f: F) -> ConductorResult<ConductorState>
    where
        F: FnOnce(ConductorState) -> ConductorResult<ConductorState>,
    {
        self.check_running()?;
        let guard = self.env.guard();
        let new_state = guard.with_commit(|txn| {
            let state: ConductorState = self.state_db.get(txn, &UnitDbKey)?.unwrap_or_default();
            let new_state = f(state)?;
            self.state_db.put(txn, &UnitDbKey, &new_state)?;
            Result::<_, ConductorError>::Ok(new_state)
        })?;
        Ok(new_state)
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
    use crate::conductor::{dna_store::RealDnaStore, ConductorHandle};
    use holochain_state::{env::EnvironmentKind, test_utils::TestEnvironment};

    /// A configurable Builder for Conductor and sometimes ConductorHandle
    #[derive(Default)]
    pub struct ConductorBuilder<DS = RealDnaStore> {
        config: ConductorConfig,
        dna_store: DS,
        keystore: Option<KeystoreSender>,
        #[cfg(test)]
        state: Option<ConductorState>,
        #[cfg(test)]
        mock_handle: Option<MockConductorHandle>,
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

            let _ = holochain_crypto::crypto_init_sodium();

            let keystore = if let Some(keystore) = self.keystore {
                keystore
            } else {
                delete_me_create_test_keystore().await
            };
            let env_path = self.config.environment_path.clone();

            let environment = EnvironmentWrite::new(
                env_path.as_ref(),
                EnvironmentKind::Conductor,
                keystore.clone(),
            )?;

            let wasm_environment =
                EnvironmentWrite::new(env_path.as_ref(), EnvironmentKind::Wasm, keystore.clone())?;

            #[cfg(test)]
            let state = self.state;

            let Self {
                dna_store, config, ..
            } = self;

            let (holochain_p2p, p2p_evt) = holochain_p2p::spawn_holochain_p2p().await?;

            let conductor = Conductor::new(
                environment,
                wasm_environment,
                dna_store,
                keystore,
                env_path,
                holochain_p2p,
            )
            .await?;

            #[cfg(test)]
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

            handle.add_dnas().await?;

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

            tokio::task::spawn(p2p_event_task(p2p_evt, handle.clone()));

            Ok(handle)
        }

        /// Pass a test keystore in, to ensure that generated test agents
        /// are actually available for signing (especially for tryorama compat)
        pub fn with_keystore(mut self, keystore: KeystoreSender) -> Self {
            self.keystore = Some(keystore);
            self
        }

        #[cfg(test)]
        /// Sets some fake conductor state for tests
        pub fn fake_state(mut self, state: ConductorState) -> Self {
            self.state = Some(state);
            self
        }

        /// Pass a mock handle in, which will be returned regardless of whatever
        /// else happens to this builder
        #[cfg(test)]
        pub fn with_mock_handle(mut self, handle: MockConductorHandle) -> Self {
            self.mock_handle = Some(handle);
            self
        }

        #[cfg(test)]
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
        pub async fn test(
            self,
            test_env: TestEnvironment,
            test_wasm_env: EnvironmentWrite,
        ) -> ConductorResult<ConductorHandle> {
            let TestEnvironment {
                env: environment,
                tmpdir,
            } = test_env;
            let keystore = environment.keystore();
            let (holochain_p2p, p2p_evt) = holochain_p2p::spawn_holochain_p2p().await?;
            let conductor = Conductor::new(
                environment,
                test_wasm_env,
                self.dna_store,
                keystore,
                tmpdir.path().to_path_buf().into(),
                holochain_p2p,
            )
            .await?;

            #[cfg(test)]
            let conductor = Self::update_fake_state(self.state, conductor).await?;

            Self::finish(conductor, self.config, p2p_evt).await
        }
    }
}

async fn p2p_event_task(
    mut p2p_evt: holochain_p2p::event::HolochainP2pEventReceiver,
    handle: ConductorHandle,
) {
    use tokio::stream::StreamExt;
    while let Some(evt) = p2p_evt.next().await {
        let cell_id = CellId::new(evt.dna_hash().clone(), evt.as_to_agent().clone());
        if let Err(e) = handle.dispatch_holochain_p2p_event(&cell_id, evt).await {
            tracing::error!(
                message = "error dispatching network event",
                error = ?e,
            );
        }
    }
    tracing::warn!("p2p_event_task has ended");
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use super::{Conductor, ConductorState};
    use crate::conductor::dna_store::MockDnaStore;
    use holochain_state::test_utils::{test_conductor_env, test_wasm_env, TestEnvironment};
    use holochain_types::test_utils::fake_cell_id;

    #[tokio::test(threaded_scheduler)]
    async fn can_update_state() {
        let TestEnvironment {
            env: environment,
            tmpdir,
        } = test_conductor_env();
        let TestEnvironment {
            env: wasm_env,
            tmpdir: _tmpdir,
        } = test_wasm_env();
        let dna_store = MockDnaStore::new();
        let keystore = environment.keystore().clone();
        let (holochain_p2p, _p2p_evt) = holochain_p2p::spawn_holochain_p2p().await.unwrap();
        let conductor = Conductor::new(
            environment,
            wasm_env,
            dna_store,
            keystore,
            tmpdir.path().to_path_buf().into(),
            holochain_p2p,
        )
        .await
        .unwrap();
        let state = conductor.get_state().await.unwrap();
        assert_eq!(state, ConductorState::default());

        let cell_id = fake_cell_id(1);
        let installed_cell = InstalledCell::new(cell_id.clone(), "handle".to_string());

        conductor
            .update_state(|mut state| {
                state
                    .inactive_apps
                    .insert("fake app".to_string(), vec![installed_cell]);
                Ok(state)
            })
            .await
            .unwrap();
        let state = conductor.get_state().await.unwrap();
        assert_eq!(
            state.inactive_apps.values().collect::<Vec<_>>()[0]
                .into_iter()
                .map(|c| c.as_id().clone())
                .collect::<Vec<_>>()
                .as_slice(),
            &[cell_id]
        );
    }

    #[tokio::test(threaded_scheduler)]
    async fn can_set_fake_state() {
        let test_env = test_conductor_env();
        let _tmpdir = test_env.tmpdir.clone();
        let TestEnvironment {
            env: wasm_env,
            tmpdir: _tmpdir,
        } = test_wasm_env();
        let state = ConductorState::default();
        let conductor = ConductorBuilder::new()
            .fake_state(state.clone())
            .test(test_env, wasm_env)
            .await
            .unwrap();
        assert_eq!(state, conductor.get_state_from_handle().await.unwrap());
    }
}
