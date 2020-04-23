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
    api::{AdminInterfaceApi, AppInterfaceApi},
    dna_store::{DnaStore, RealDnaStore},
    error::ConductorError,
    handle::ConductorHandleImpl,
    manager::{spawn_task_manager, ManagedTaskAdd, TaskManagerRunHandle},
    state::ConductorState,
};
use crate::conductor::{
    api::error::{ConductorApiError, ConductorApiResult},
    cell::{Cell, NetSender},
    config::ConductorConfig,
    dna_store::MockDnaStore,
    error::ConductorResult,
    handle::ConductorHandle,
};
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use sx_state::{
    db,
    env::{Environment, ReadManager},
    exports::SingleStore,
    prelude::WriteManager,
    typed::{Kv, UnitDbKey},
};
use sx_types::{
    cell::{CellHandle, CellId},
    prelude::*,
    shims::Keystore,
};
use tokio::sync::{mpsc, RwLock};
use tracing::*;

pub use builder::*;

/// Conductor-specific Cell state, this can probably be stored in a database.
/// Hypothesis: If nothing remains in this struct, then the Conductor state is
/// essentially immutable, and perhaps we just throw it out and make a new one
/// when we need to load new config, etc.
pub struct CellState {
    /// Whether or not we should call any methods on the cell
    _active: bool,
}

/// An [Cell] tracked by a Conductor, along with some [CellState]
struct CellItem {
    cell: Cell,
    _state: CellState,
}

pub type StopBroadcaster = tokio::sync::broadcast::Sender<()>;
pub type StopReceiver = tokio::sync::broadcast::Receiver<()>;

/// A Conductor is a group of [Cell]s
pub struct Conductor<DS = RealDnaStore>
where
    DS: DnaStore,
{
    /// The collection of cells associated with this Conductor
    cells: HashMap<CellId, CellItem>,

    /// The LMDB environment for persisting state related to this Conductor
    env: Environment,

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

    /// Placeholder. A way to get a Keystore from an AgentHash.
    _agent_keys: HashMap<AgentHash, Keystore>,

    /// Channel on which to send info about tasks we want to manage
    managed_task_add_sender: mpsc::Sender<ManagedTaskAdd>,

    /// broadcast channel sender, used to end all managed tasks
    managed_task_stop_broadcaster: StopBroadcaster,

    /// The main task join handle to await on.
    /// The conductor is intended to live as long as this task does.
    task_manager_run_handle: Option<TaskManagerRunHandle>,

    /// Placeholder for what will be the real DNA/Wasm cache
    dna_store: DS,
}

impl Conductor {
    /// Create a conductor builder
    pub fn builder() -> ConductorBuilder {
        ConductorBuilder::new()
    }
}

impl<DS> Conductor<DS>
where
    DS: DnaStore + 'static,
{
    async fn new(env: Environment, dna_store: DS) -> ConductorResult<Self> {
        let db: SingleStore = *env.dbs().await?.get(&db::CONDUCTOR_STATE)?;
        let (task_tx, task_manager_run_handle) = spawn_task_manager();
        let task_manager_run_handle = Some(task_manager_run_handle);
        let (stop_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        Ok(Self {
            env,
            state_db: Kv::new(db)?,
            cells: HashMap::new(),
            _handle_map: HashMap::new(),
            _agent_keys: HashMap::new(),
            shutting_down: false,
            managed_task_add_sender: task_tx,
            managed_task_stop_broadcaster: stop_tx,
            task_manager_run_handle,
            admin_websocket_ports: Vec::new(),
            dna_store,
        })
    }

    // NOTE: This could lead to a potential deadlock because AdminInterfaceApi contains
    // the Conductor handle (from where this could be called)
    #[allow(dead_code)]
    async fn spawn_admin_interface<Api: AdminInterfaceApi>(
        &mut self,
        _api: Api,
    ) -> ConductorResult<()> {
        self.check_running()?;
        unimplemented!()
    }

    // NOTE: This could lead to a potential deadlock because AdminInterfaceApi contains
    // the Conductor handle (from where this could be called)
    /// The common way to spawn a new app interface, whether read from
    /// ConductorState on startup, or generated on-the-fly by an admin method
    #[allow(dead_code)]
    async fn spawn_app_interface<Api: AppInterfaceApi>(
        &mut self,
        _api: Api,
    ) -> ConductorResult<()> {
        self.check_running()?;
        unimplemented!()
    }

    // FIXME: remove allow once we actually use this function
    #[allow(dead_code)]
    async fn update_state<F: Send>(&self, f: F) -> ConductorResult<ConductorState>
    where
        F: FnOnce(ConductorState) -> ConductorResult<ConductorState>,
    {
        self.check_running()?;
        let guard = self.env.guard().await;
        let mut writer = guard.writer()?;
        let state: ConductorState = self.state_db.get(&writer, &UnitDbKey)?.unwrap_or_default();
        let new_state = f(state)?;
        self.state_db.put(&mut writer, &UnitDbKey, &new_state)?;
        writer.commit()?;
        Ok(new_state)
    }
}

// TODO: @freesig: is there a reason for the separate impl blocks here?
impl<DS> Conductor<DS>
where
    DS: DnaStore + 'static,
{
    pub(super) fn cell_by_id(&self, cell_id: &CellId) -> ConductorApiResult<&Cell> {
        let item = self
            .cells
            .get(cell_id)
            .ok_or_else(|| ConductorApiError::CellMissing(cell_id.clone()))?;
        Ok(&item.cell)
    }

    fn tx_network(&self) -> &NetSender {
        unimplemented!()
    }

    /// Sends a JoinHandle to the TaskManager task to be managed
    async fn manage_task(&mut self, handle: ManagedTaskAdd) -> ConductorResult<()> {
        self.managed_task_add_sender
            .send(handle)
            .await
            .map_err(|e| ConductorError::SubmitTaskError(format!("{}", e)))
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

    /// Returns a port that was chosen by the OS
    fn get_arbitrary_admin_websocket_port(&self) -> Option<u16> {
        self.admin_websocket_ports.get(0).copied()
    }

    #[allow(dead_code)]
    async fn get_state(&self) -> ConductorResult<ConductorState> {
        let guard = self.env.guard().await;
        let reader = guard.reader()?;
        Ok(self.state_db.get(&reader, &UnitDbKey)?.unwrap_or_default())
    }

    fn dna_store(&self) -> &DS {
        &self.dna_store
    }

    fn dna_store_mut(&mut self) -> &mut DS {
        &mut self.dna_store
    }

    fn add_admin_port(&mut self, port: u16) {
        self.admin_websocket_ports.push(port);
    }

    fn shutdown(&mut self) {
        self.shutting_down = true;
        self.managed_task_stop_broadcaster
            .send(())
            .map(|_| ())
            .unwrap_or_else(|e| {
                error!(?e, "Couldn't broadcast stop signal to managed tasks!");
            })
    }

    fn get_wait_handle(&mut self) -> Option<TaskManagerRunHandle> {
        self.task_manager_run_handle.take()
    }

    pub fn into_handle(self) -> ConductorHandle {
        Arc::new(ConductorHandleImpl::from(RwLock::new(self)))
    }
}

type ConductorStateDb = Kv<UnitDbKey, ConductorState>;

mod builder {

    use super::*;
    use crate::conductor::{
        api::RealAdminInterfaceApi,
        config::AdminInterfaceConfig,
        dna_store::RealDnaStore,
        interface::{
            error::InterfaceResult,
            websocket::{spawn_admin_interface_task, spawn_websocket_listener},
            InterfaceDriver,
        },
        manager::{keep_alive_task, ManagedTaskHandle},
        ConductorHandle,
    };
    use futures::future;
    use sx_state::{env::EnvironmentKind, test_utils::test_conductor_env};
    use tokio::sync::RwLockWriteGuard;

    #[derive(Default)]
    pub struct ConductorBuilder<DS = RealDnaStore> {
        dna_store: DS,
    }

    impl ConductorBuilder {
        pub fn new() -> Self {
            ConductorBuilder {
                dna_store: RealDnaStore::new(),
            }
        }
    }

    impl ConductorBuilder<MockDnaStore> {
        pub fn with_mock_dna_store(dna_store: MockDnaStore) -> ConductorBuilder<MockDnaStore> {
            ConductorBuilder { dna_store }
        }
    }

    impl<DS> ConductorBuilder<DS>
    where
        DS: DnaStore + 'static,
    {
        pub async fn with_config(
            self,
            config: ConductorConfig,
        ) -> ConductorResult<ConductorHandle> {
            let env_path = config.environment_path;
            let environment = Environment::new(env_path.as_ref(), EnvironmentKind::Conductor)?;
            let conductor = Conductor::new(environment, self.dna_store).await?;
            let stop_tx = conductor.managed_task_stop_broadcaster.clone();
            let conductor_mutex = RwLock::new(conductor);
            let mut conductor_lock = conductor_mutex.write().await;
            let conductor_handle: ConductorHandle =
                Arc::new(ConductorHandleImpl::from(conductor_mutex));
            let admin_api = RealAdminInterfaceApi::new(conductor_handle.clone());

            setup_admin_interfaces_from_config(
                &mut conductor_lock,
                admin_api,
                stop_tx,
                config.admin_interfaces.unwrap_or_default(),
            )
            .await?;

            Ok(conductor_handle)
        }

        pub async fn test(self) -> ConductorResult<ConductorHandle> {
            let environment = test_conductor_env();
            let conductor = Conductor::new(environment, self.dna_store).await?;
            let conductor_handle: ConductorHandle = conductor.into_handle();
            Ok(conductor_handle)
        }
    }

    /// Spawn all admin interface tasks, register them with the TaskManager,
    /// and modify the conductor accordingly, based on the config passed in
    async fn setup_admin_interfaces_from_config<DS>(
        conductor_lock: &mut RwLockWriteGuard<'_, Conductor<DS>>,
        admin_api: RealAdminInterfaceApi,
        stop_tx: StopBroadcaster,
        configs: Vec<AdminInterfaceConfig>,
    ) -> ConductorResult<()>
    where
        DS: DnaStore + 'static,
    {
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
        // and throw away the errors after logging them
        let handles: Vec<_> = future::join_all(configs.into_iter().map(spawn_from_config))
            .await
            .into_iter()
            // Log errors
            .inspect(|result| {
                if let Err(ref e) = result {
                    error!(error = e as &dyn Error, "Admin interface failed to parse");
                }
            })
            // Throw away errors
            .filter_map(Result::ok)
            .collect();

        {
            let mut ports = Vec::new();

            // First, register the keepalive task, to ensure the conductor doesn't shut down
            // in the absence of other "real" tasks
            conductor_lock
                .manage_task(ManagedTaskAdd::dont_handle(tokio::spawn(keep_alive_task(
                    stop_tx.subscribe(),
                ))))
                .await?;

            // Now that tasks are spawned, register them with the TaskManager
            for (port, handle) in handles {
                ports.push(port);
                conductor_lock
                    .manage_task(ManagedTaskAdd::new(
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
                conductor_lock.add_admin_port(p);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {

    use super::{Conductor, ConductorState};
    use crate::conductor::{dna_store::MockDnaStore, state::CellConfig};
    use sx_state::test_utils::test_conductor_env;

    #[tokio::test]
    async fn can_update_state() {
        let environment = test_conductor_env();
        let dna_store = MockDnaStore::new();
        let conductor = Conductor::new(environment, dna_store).await.unwrap();
        let state = conductor.get_state().await.unwrap();
        assert_eq!(state, ConductorState::default());

        let cell_config = CellConfig {
            id: "".to_string(),
            agent: "".to_string(),
            dna: "".to_string(),
        };

        conductor
            .update_state(|mut state| {
                state.cells.push(cell_config.clone());
                Ok(state)
            })
            .await
            .unwrap();
        let state = conductor.get_state().await.unwrap();
        assert_eq!(state.cells, [cell_config]);
    }
}
