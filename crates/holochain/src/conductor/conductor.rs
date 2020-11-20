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
        SignalBroadcaster,
    },
    manager::{
        keep_alive_task, spawn_task_manager, ManagedTaskAdd, ManagedTaskHandle,
        TaskManagerRunHandle,
    },
    paths::EnvironmentRootPath,
    state::AppInterfaceId,
    state::ConductorState,
    CellError,
};
use crate::conductor::p2p_store::{AgentKv, AgentKvKey};
use crate::{
    conductor::{
        api::error::ConductorApiResult, cell::Cell, config::ConductorConfig,
        dna_store::MockDnaStore, error::ConductorResult, handle::ConductorHandle,
    },
    core::signal::Signal,
    core::state::{source_chain::SourceChainBuf, wasm::WasmBuf},
};
pub use builder::*;
use fallible_iterator::FallibleIterator;
use futures::future::{self, TryFutureExt};
use holo_hash::DnaHash;
use holochain_keystore::{
    lair_keystore::spawn_lair_keystore, test_keystore::spawn_test_keystore, KeystoreSender,
    KeystoreSenderExt,
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
    app::{InstalledApp, InstalledAppId, InstalledCell, MembraneProof},
    cell::CellId,
    dna::{wasm::DnaWasmHashed, DnaFile},
};
use holochain_zome_types::entry_def::EntryDef;
use kitsune_p2p::agent_store::AgentInfoSigned;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
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

    /// Collection of signal broadcasters per app interface, keyed by id
    app_interface_signal_broadcasters:
        HashMap<AppInterfaceId, tokio::sync::broadcast::Sender<Signal>>,

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
        let (signal_broadcaster, _r) = tokio::sync::broadcast::channel(SIGNAL_BUFFER_SIZE);
        let stop_rx = self.managed_task_stop_broadcaster.subscribe();
        let (port, task) =
            spawn_app_interface_task(port, app_api, signal_broadcaster.clone(), stop_rx)
                .await
                .map_err(Box::new)?;
        // TODO: RELIABILITY: Handle this task by restarting it if it fails and log the error
        self.manage_task(ManagedTaskAdd::dont_handle(task)).await?;
        self.app_interface_signal_broadcasters
            .insert(interface_id, signal_broadcaster);
        Ok(port)
    }

    pub(super) fn signal_broadcaster(&self) -> SignalBroadcaster {
        SignalBroadcaster::new(
            self.app_interface_signal_broadcasters
                .values()
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
    ) -> ConductorResult<Vec<Result<Vec<Cell>, CreateAppError>>> {
        // Only create the active apps
        let active_apps = self.get_state().await?.active_apps;

        // Data required to create apps
        let root_env_dir = self.root_env_dir.clone();
        let keystore = self.keystore.clone();

        // Closure for creating all cells in an app
        let tasks = active_apps.into_iter().map(
            move |(installed_app_id, cells): (InstalledAppId, Vec<InstalledCell>)| {
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
            let is_active = state.active_apps.contains_key(&app.installed_app_id);
            let is_inactive = state
                .inactive_apps
                .insert(app.installed_app_id.clone(), app.cell_data)
                .is_some();
            if is_active || is_inactive {
                Err(ConductorError::AppAlreadyInstalled(app.installed_app_id))
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
            let cell_data = state
                .inactive_apps
                .remove(&installed_app_id)
                .ok_or_else(|| ConductorError::AppNotInstalled(installed_app_id.clone()))?;
            state.active_apps.insert(installed_app_id, cell_data);
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
                    let cell_ids = state
                        .active_apps
                        .remove(&installed_app_id)
                        .ok_or_else(|| ConductorError::AppNotActive(installed_app_id.clone()))?;
                    state.inactive_apps.insert(installed_app_id, cell_ids);
                    Ok(state)
                }
            })
            .await?;
        Ok(state
            .inactive_apps
            .get(&installed_app_id)
            .expect("This app was just put here")
            .clone()
            .into_iter()
            .map(|c| c.into_id())
            .collect())
    }

    /// Add fully constructed cells to the cell map in the Conductor
    pub(super) fn add_cells(&mut self, cells: Vec<Cell>) {
        for cell in cells {
            let cell_id = cell.id().clone();
            tracing::info!(?cell_id, "ADD CELL");
            self.cells.insert(
                cell_id,
                CellItem {
                    cell,
                    _state: CellState { _active: false },
                },
            );
        }
    }

    pub(super) fn initialize_cell_workflows(&mut self) {
        for cell in self.cells.values_mut() {
            cell.cell.initialize_workflows();
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
            .get_all()?
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

    pub(super) fn put_agent_info_signed(
        &self,
        agent_info_signed: kitsune_p2p::agent_store::AgentInfoSigned,
    ) -> ConductorResult<()> {
        let environ = self.p2p_env.clone();
        let p2p_kv = AgentKv::new(environ.clone().into())?;
        let env = environ.guard();
        Ok(env.with_commit(|writer| {
            p2p_kv.as_store_ref().put(
                writer,
                &(&agent_info_signed).try_into()?,
                &agent_info_signed,
            )
        })?)
    }

    pub(super) fn get_agent_info_signed(
        &self,
        kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
        kitsune_agent: Arc<kitsune_p2p::KitsuneAgent>,
    ) -> ConductorResult<Option<AgentInfoSigned>> {
        let environ = self.p2p_env.clone();

        let p2p_kv = AgentKv::new(environ.clone().into())?;
        let env = environ.guard();

        env.with_commit(|writer| {
            let res = p2p_kv
                .as_store_ref()
                .get(writer, &(&*kitsune_space, &*kitsune_agent).into())?;

            let res = match res {
                None => return Ok(None),
                Some(res) => res,
            };

            let info = kitsune_p2p::agent_store::AgentInfo::try_from(&res)?;
            let now: u64 = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            if info.signed_at_ms() + info.expires_after_ms() <= now {
                p2p_kv
                    .as_store_ref()
                    .delete(writer, &(&*kitsune_space, &*kitsune_agent).into())?;
                return Ok(None);
            }

            Ok(Some(res))
        })
    }

    pub(super) fn query_agent_info_signed(
        &self,
        _kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
    ) -> ConductorResult<Vec<AgentInfoSigned>> {
        let environ = self.p2p_env.clone();

        let p2p_kv = AgentKv::new(environ.clone().into())?;
        let env = environ.guard();

        let mut out = Vec::new();
        env.with_commit(|writer| {
            let mut expired = Vec::new();

            {
                let mut iter = p2p_kv.as_store_ref().iter(writer)?;

                let now: u64 = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;

                loop {
                    match iter.next() {
                        Ok(Some((k, v))) => {
                            let info = kitsune_p2p::agent_store::AgentInfo::try_from(&v)?;
                            if info.signed_at_ms() + info.expires_after_ms() <= now {
                                expired.push(AgentKvKey::from(k));
                            } else {
                                out.push(v);
                            }
                        }
                        Ok(None) => break,
                        Err(e) => return Err(e.into()),
                    }
                }
            }

            if !expired.is_empty() {
                for exp in expired {
                    p2p_kv.as_store_ref().delete(writer, &exp)?;
                }
            }

            ConductorResult::Ok(())
        })?;

        Ok(out)
    }

    pub(super) async fn put_wasm(
        &self,
        dna: DnaFile,
    ) -> ConductorResult<Vec<(EntryDefBufferKey, EntryDef)>> {
        let environ = self.wasm_env.clone();
        let wasm = environ.get_db(&*holochain_state::db::WASM)?;
        let dna_def_db = environ.get_db(&*holochain_state::db::DNA_DEF)?;
        let entry_def_db = environ.get_db(&*holochain_state::db::ENTRY_DEF)?;

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

    #[cfg(any(test, feature = "test_utils"))]
    pub(super) async fn get_state_from_handle(&self) -> ConductorResult<ConductorState> {
        self.get_state().await
    }

    #[cfg(any(test, feature = "test_utils"))]
    pub(super) fn get_p2p_env(&self) -> EnvironmentWrite {
        self.p2p_env.clone()
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
            app_interface_signal_broadcasters: HashMap::new(),
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
    use holochain_state::{env::EnvironmentKind, test_utils::TestEnvironments};

    /// A configurable Builder for Conductor and sometimes ConductorHandle
    #[derive(Default)]
    pub struct ConductorBuilder<DS = RealDnaStore> {
        config: ConductorConfig,
        dna_store: DS,
        keystore: Option<KeystoreSender>,
        #[cfg(any(test, feature = "test_utils"))]
        state: Option<ConductorState>,
        #[cfg(any(test, feature = "test_utils"))]
        mock_handle: Option<MockConductorHandleT>,
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
            let (holochain_p2p, p2p_evt) =
                holochain_p2p::spawn_holochain_p2p(network_config).await?;

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

            handle.add_dnas().await?;

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
        pub async fn test(self, envs: &TestEnvironments) -> ConductorResult<ConductorHandle> {
            let keystore = envs.conductor().keystore();
            let (holochain_p2p, p2p_evt) =
                holochain_p2p::spawn_holochain_p2p(self.config.network.clone().unwrap_or_default())
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
    use holochain_state::test_utils::test_environments;
    use holochain_types::test_utils::fake_cell_id;
    use matches::assert_matches;

    #[tokio::test(threaded_scheduler)]
    async fn can_update_state() {
        let envs = test_environments();
        let dna_store = MockDnaStore::new();
        let keystore = envs.conductor().keystore().clone();
        let holochain_p2p = holochain_p2p::stub_network().await;
        let conductor = Conductor::new(
            envs.conductor(),
            envs.wasm(),
            envs.p2p(),
            dna_store,
            keystore,
            envs.tempdir().path().to_path_buf().into(),
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

    /// App can't be installed if another app is already installed under the
    /// same InstalledAppId
    #[tokio::test(threaded_scheduler)]
    async fn app_ids_are_unique() {
        let environments = test_environments();
        let dna_store = MockDnaStore::new();
        let holochain_p2p = holochain_p2p::stub_network().await;
        let mut conductor = Conductor::new(
            environments.conductor(),
            environments.wasm(),
            environments.p2p(),
            dna_store,
            environments.keystore().clone(),
            environments.tempdir().path().to_path_buf().into(),
            holochain_p2p,
        )
        .await
        .unwrap();

        let cell_id = fake_cell_id(1);
        let installed_cell = InstalledCell::new(cell_id.clone(), "handle".to_string());
        let app = InstalledApp {
            installed_app_id: "id".to_string(),
            cell_data: vec![installed_cell],
        };

        conductor.add_inactive_app_to_db(app.clone()).await.unwrap();

        assert_matches!(
            conductor.add_inactive_app_to_db(app.clone()).await,
            Err(ConductorError::AppAlreadyInstalled(id))
            if id == "id".to_string()
        );

        //- it doesn't matter whether the app is active or inactive
        conductor
            .activate_app_in_db("id".to_string())
            .await
            .unwrap();

        assert_matches!(
            conductor.add_inactive_app_to_db(app.clone()).await,
            Err(ConductorError::AppAlreadyInstalled(id))
            if id == "id".to_string()
        );
    }

    #[tokio::test(threaded_scheduler)]
    async fn can_set_fake_state() {
        let envs = test_environments();
        let state = ConductorState::default();
        let conductor = ConductorBuilder::new()
            .fake_state(state.clone())
            .test(&envs)
            .await
            .unwrap();
        assert_eq!(state, conductor.get_state_from_handle().await.unwrap());
    }
}
