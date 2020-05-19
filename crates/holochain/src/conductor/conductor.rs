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
    dna_store::{DnaStore, RealDnaStore},
    error::ConductorError,
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
        api::error::{ConductorApiError, ConductorApiResult},
        cell::Cell,
        config::ConductorConfig,
        dna_store::MockDnaStore,
        error::ConductorResult,
        handle::ConductorHandle,
    },
    core::state::{source_chain::SourceChainBuf, wasm::WasmBuf},
};
use holochain_keystore::{
    test_keystore::{spawn_test_keystore, MockKeypair},
    KeystoreSender,
};
use holochain_state::{
    buffer::BufferedStore,
    db,
    env::{EnvironmentWrite, ReadManager},
    exports::SingleStore,
    prelude::*,
    typed::{Kv, UnitDbKey},
};
use holochain_types::{
    cell::{CellHandle, CellId},
    dna::DnaFile,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::*;

pub use builder::*;
use futures::future::{self, TryFutureExt};
use holochain_serialized_bytes::SerializedBytes;

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

    /// Placeholder. A way to look up a Cell from its app-specific handle.
    _handle_map: HashMap<CellHandle, CellId>,

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
    pub(super) fn cell_by_id(&self, cell_id: &CellId) -> ConductorApiResult<&Cell> {
        debug!(cells_map = ?self.cells.keys().collect::<Vec<_>>());
        let item = self
            .cells
            .get(cell_id)
            .ok_or_else(|| ConductorApiError::CellMissing(cell_id.clone()))?;
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

    pub(super) fn get_wait_handle(&mut self) -> Option<TaskManagerRunHandle> {
        self.task_manager_run_handle.take()
    }

    /// Spawn all admin interface tasks, register them with the TaskManager,
    /// and modify the conductor accordingly, based on the config passed in
    pub(super) async fn add_admin_interfaces_via_handle(
        &mut self,
        handle: ConductorHandle,
        configs: Vec<AdminInterfaceConfig>,
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

    /// Create the cells from the db
    pub(super) async fn genesis_cells(
        &self,
        cell_ids_with_proofs: Vec<(CellId, Option<SerializedBytes>)>,
        conductor_handle: ConductorHandle,
    ) -> ConductorResult<Vec<CellId>> {
        let root_env_dir = self.root_env_dir.clone();
        let keystore = self.keystore.clone();

        let cells_tasks = cell_ids_with_proofs
            .into_iter()
            .map(move |(cell_id, proof)| {
                let root_env_dir = std::path::PathBuf::from(root_env_dir.clone());
                tokio::spawn(Cell::genesis(
                    cell_id.clone(),
                    conductor_handle.clone(),
                    root_env_dir,
                    keystore.clone(),
                    proof,
                ))
                .map_err(|e| CellError::from(e))
                .and_then(|result| async { result.map(|env| (cell_id, env)) })
            })
            .collect::<Vec<_>>();
        let (success, errors): (Vec<_>, Vec<_>) = futures::future::join_all(cells_tasks)
            .await
            .into_iter()
            .partition(Result::is_ok);

        // unwrap safe because of the partition
        let success = success.into_iter().map(Result::unwrap);

        // If there was errors, cleanup and return the errors
        if !errors.is_empty() {
            for (_, state_env) in success {
                state_env.remove().await?;
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
            Ok(success.map(|(cell_id, _)| cell_id).collect())
        }
    }

    /// Create the cells from the db
    pub(super) async fn create_cells(
        &self,
        conductor_handle: ConductorHandle,
    ) -> ConductorResult<Vec<Cell>> {
        let cell_ids = self.get_state().await?.cell_ids;
        let root_env_dir = self.root_env_dir.clone();
        let keystore = self.keystore.clone();

        // Only create cells not already created
        let cells_to_create = cell_ids
            .into_iter()
            .filter(|cell_id| !self.cells.contains_key(cell_id));

        let cells_tasks = cells_to_create
            .map(move |cell_id| {
                let root_env_dir = std::path::PathBuf::from(root_env_dir.clone());
                Cell::create(
                    cell_id,
                    conductor_handle.clone(),
                    root_env_dir,
                    keystore.clone(),
                )
            })
            .collect::<Vec<_>>();
        let (success, errors): (Vec<_>, Vec<_>) = futures::future::join_all(cells_tasks)
            .await
            .into_iter()
            .partition(Result::is_ok);

        // unwrap safe because of the partition
        let success = success.into_iter().map(Result::unwrap);
        // If there was errors, cleanup and return the errors
        if !errors.is_empty() {
            for cell in success {
                cell.cleanup().await?;
            }
            // match needed to avoid Debug requirement on unwrap_err
            let errors = errors
                .into_iter()
                .map(|e| match e {
                    Err(e) => e,
                    Ok(_) => unreachable!("Safe because of the partition"),
                })
                .collect();
            Err(ConductorError::CreateCellsFailed { errors })
        } else {
            // No errors so return the cells
            Ok(success.collect())
        }
    }

    /// Register CellIds in the database
    pub(super) async fn add_cell_ids_to_db(
        &mut self,
        mut cell_ids: Vec<CellId>,
    ) -> ConductorResult<()> {
        self.update_state(move |mut state| {
            state.cell_ids.append(&mut cell_ids);
            // Make sure they are unique
            state.cell_ids = state
                .cell_ids
                .iter()
                .cloned()
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();
            Ok(state)
        })
        .await?;
        Ok(())
    }

    /// Add fully constructed cells to to the cell map in the Conductor
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

    pub(super) async fn put_wasm(&mut self, dna: DnaFile) -> ConductorResult<()> {
        let environ = &self.wasm_env;
        let env = environ.guard().await;
        let wasm = environ.get_db(&*holochain_state::db::WASM)?;
        let reader = env.reader()?;

        let mut wasm_buf = WasmBuf::new(&reader, wasm)?;
        // TODO: PERF: This loop might be slow
        for (wasm_hash, dna_wasm) in dna.code().clone().into_iter() {
            if let None = wasm_buf.get(&wasm_hash.into()).await? {
                wasm_buf.put(dna_wasm).await?;
            }
        }

        // write the db
        env.with_commit(|writer| wasm_buf.flush_to_txn(writer))?;

        Ok(())
    }

    pub(super) async fn dump_cell_state(&self, cell_id: &CellId) -> ConductorApiResult<String> {
        let cell = self.cell_by_id(cell_id)?;
        let arc = cell.state_env();
        let env = arc.guard().await;
        let reader = env.reader()?;
        let source_chain = SourceChainBuf::new(&reader, &env)?;
        Ok(source_chain.dump_as_json().await?)
    }

    #[cfg(test)]
    pub(super) async fn get_state_from_handle(&self) -> ConductorResult<ConductorState> {
        self.get_state().await
    }
}

// -- TODO - delete this helper when we have a real keystore -- //

async fn delete_me_create_test_keystore() -> KeystoreSender {
    use std::convert::TryFrom;
    let _ = holochain_crypto::crypto_init_sodium();
    let mut keystore = spawn_test_keystore(vec![
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
    ) -> ConductorResult<Self> {
        let db: SingleStore = env.get_db(&db::CONDUCTOR_STATE)?;
        let (task_tx, task_manager_run_handle) = spawn_task_manager();
        let task_manager_run_handle = Some(task_manager_run_handle);
        let (stop_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        Ok(Self {
            env,
            wasm_env,
            state_db: Kv::new(db)?,
            cells: HashMap::new(),
            _handle_map: HashMap::new(),
            shutting_down: false,
            managed_task_add_sender: task_tx,
            managed_task_stop_broadcaster: stop_tx,
            task_manager_run_handle,
            admin_websocket_ports: Vec::new(),
            dna_store,
            keystore,
            root_env_dir,
        })
    }

    // FIXME: remove allow once we actually use this function
    #[allow(dead_code)]
    async fn get_state(&self) -> ConductorResult<ConductorState> {
        let guard = self.env.guard().await;
        let reader = guard.reader()?;
        Ok(self.state_db.get(&reader, &UnitDbKey)?.unwrap_or_default())
    }

    async fn update_state<F: Send>(&self, f: F) -> ConductorResult<ConductorState>
    where
        F: FnOnce(ConductorState) -> ConductorResult<ConductorState>,
    {
        self.check_running()?;
        let guard = self.env.guard().await;
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

type ConductorStateDb = Kv<UnitDbKey, ConductorState>;

mod builder {

    use super::*;
    use crate::conductor::{dna_store::RealDnaStore, ConductorHandle};
    use holochain_state::{env::EnvironmentKind, test_utils::TestEnvironment};

    /// A configurable Builder for Conductor and sometimes ConductorHandle
    #[derive(Default)]
    pub struct ConductorBuilder<DS = RealDnaStore> {
        config: ConductorConfig,
        dna_store: DS,
        #[cfg(test)]
        state: Option<ConductorState>,
    }

    impl ConductorBuilder<RealDnaStore> {
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
            let keystore = delete_me_create_test_keystore().await;
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

            let conductor =
                Conductor::new(environment, wasm_environment, dna_store, keystore, env_path)
                    .await?;

            #[cfg(test)]
            let conductor = Self::update_fake_state(state, conductor).await?;

            Self::finish(conductor, config).await
        }

        async fn finish(
            conductor: Conductor<DS>,
            conductor_config: ConductorConfig,
        ) -> ConductorResult<ConductorHandle> {
            // Get data before handle
            let keystore = conductor.keystore.clone();

            // Create handle
            let handle: ConductorHandle = Arc::new(ConductorHandleImpl::from((
                RwLock::new(conductor),
                keystore,
            )));

            handle.setup_cells(handle.clone()).await?;

            // Create admin interfaces
            if let Some(configs) = conductor_config.admin_interfaces {
                handle
                    .add_admin_interfaces_via_handle(handle.clone(), configs)
                    .await?;
            }

            Ok(handle)
        }

        #[cfg(test)]
        /// Sets some fake conductor state for tests
        pub fn fake_state(mut self, state: ConductorState) -> Self {
            self.state = Some(state);
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
            let conductor = Conductor::new(
                environment,
                test_wasm_env,
                self.dna_store,
                keystore,
                tmpdir.path().to_path_buf().into(),
            )
            .await?;

            #[cfg(test)]
            let conductor = Self::update_fake_state(self.state, conductor).await?;

            Self::finish(conductor, self.config).await
        }
    }
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
        let conductor = Conductor::new(
            environment,
            wasm_env,
            dna_store,
            keystore,
            tmpdir.path().to_path_buf().into(),
        )
        .await
        .unwrap();
        let state = conductor.get_state().await.unwrap();
        assert_eq!(state, ConductorState::default());

        let cell_id = fake_cell_id("dr. cell");

        conductor
            .update_state(|mut state| {
                state.cell_ids.push(cell_id.clone());
                Ok(state)
            })
            .await
            .unwrap();
        let state = conductor.get_state().await.unwrap();
        assert_eq!(state.cell_ids, [cell_id]);
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
