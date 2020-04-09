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
    error::ConductorError,
    manager::{spawn_task_manager, ManagedTaskAdd, ManagedTaskHandle, TaskManagerRunHandle},
    state::ConductorState,
};
use crate::conductor::{
    api::error::{ConductorApiError, ConductorApiResult},
    cell::{Cell, NetSender},
    config::ConductorConfig,
    error::ConductorResult,
};
pub use builder::*;
use derive_more::{AsRef, Deref, From};
use std::collections::HashMap;
use std::{error::Error, sync::Arc};
use sx_state::{
    db,
    env::{Environment, ReadManager},
    exports::SingleStore,
    prelude::WriteManager,
    typed::{Kv, UnitDbKey},
};
use sx_types::{
    agent::AgentId,
    cell::{CellHandle, CellId},
    shims::Keystore,
};
use tokio::sync::{mpsc, RwLock};
use tracing::*;

/// Conductor-specific Cell state, this can probably be stored in a database.
/// Hypothesis: If nothing remains in this struct, then the Conductor state is
/// essentially immutable, and perhaps we just throw it out and make a new one
/// when we need to load new config, etc.
pub struct CellState {
    /// Whether or not we should call any methods on the cell
    _active: bool,
}

///
struct CellItem {
    cell: Cell,
    _state: CellState,
}

// TODO: figure out what we need to track admin interfaces, if anything
struct AdminInterfaceHandle(());

impl AdminInterfaceHandle {
    async fn stop(&mut self) {
        unimplemented!()
    }
}

pub type StopBroadcaster = tokio::sync::broadcast::Sender<()>;
pub type StopReceiver = tokio::sync::broadcast::Receiver<()>;

#[derive(Clone, From, AsRef, Deref)]
pub struct ConductorHandle(Arc<RwLock<Conductor>>);

impl ConductorHandle {
    /// End all tasks run by the Conductor.
    pub async fn shutdown(self) {
        let mut conductor = self.0.write().await;
        conductor.shutdown().await;
    }

    pub async fn wait(&self) -> Result<(), tokio::task::JoinError> {
        // TODO: TEST: Make sure the write lock is not held for the await
        let task_manager_run_handle = {
            let mut conductor = self.0.write().await;
            conductor.wait()
        };
        if let Some(handle) = task_manager_run_handle {
            handle.await?;
        } else {
            warn!("Tried to await the task manager run handle but there was none");
        }
        Ok(())
    }
}

/// A Conductor is a group of [Cell]s
pub struct Conductor {
    // tx_network: NetSender,
    cells: HashMap<CellId, CellItem>,
    env: Environment,
    state_db: ConductorStateDb,
    shutting_down: bool,
    _handle_map: HashMap<CellHandle, CellId>,
    _agent_keys: HashMap<AgentId, Keystore>,

    /// oneshot senders used to end various managed tasks
    // TODO: define message type, spawn task that takes receiver and select!s
    // over FuturesUnordered and the receiver, etc.
    managed_task_add_sender: mpsc::Sender<ManagedTaskAdd>,

    /// broadcast channel sender, used to end all managed tasks
    managed_task_stop_sender: StopBroadcaster,

    /// The main task join handle to await on
    task_manager_run_handle: Option<TaskManagerRunHandle>,
}

impl Conductor {
    async fn new(env: Environment) -> ConductorResult<Conductor> {
        let db: SingleStore = *env.dbs().await?.get(&db::CONDUCTOR_STATE)?;
        // TODO: move task_rx into TaskManager
        let (task_tx, task_manager_run_handle) = spawn_task_manager();
        let task_manager_run_handle = Some(task_manager_run_handle);
        let (stop_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        Ok(Conductor {
            env,
            state_db: Kv::new(db)?,
            cells: HashMap::new(),
            _handle_map: HashMap::new(),
            _agent_keys: HashMap::new(),
            shutting_down: false,
            managed_task_add_sender: task_tx,
            managed_task_stop_sender: stop_tx,
            task_manager_run_handle,
        })
    }

    pub fn build() -> ConductorBuilder {
        ConductorBuilder::new()
    }

    pub(crate) fn cell_by_id(&self, cell_id: &CellId) -> ConductorApiResult<&Cell> {
        let item = self
            .cells
            .get(cell_id)
            .ok_or_else(|| ConductorApiError::CellMissing(cell_id.clone()))?;
        Ok(&item.cell)
    }

    pub(crate) fn tx_network(&self) -> &NetSender {
        unimplemented!()
    }

    /// Sends a JoinHandle to the TaskManager task to be managed
    pub(crate) async fn manage_task(&mut self, handle: ManagedTaskAdd) -> ConductorResult<()> {
        self.managed_task_add_sender
            .send(handle)
            .await
            .map_err(|e| ConductorError::SubmitTaskError(format!("{}", e)))
    }

    /// A gate to put at the top of public functions to ensure that work is not
    /// attempted after a shutdown has been issued
    // TEST: that this works
    fn check_running(&self) -> ConductorResult<()> {
        if self.shutting_down {
            Err(ConductorError::ShuttingDown)
        } else {
            Ok(())
        }
    }

    async fn shutdown(&mut self) -> () {
        self.shutting_down = true;
        self.managed_task_stop_sender
            .send(())
            .map_err(|e| {
                error!(?e, "Couldn't broadcast stop signal to managed tasks!");
            })
            .ok();
    }

    pub fn wait(&mut self) -> Option<TaskManagerRunHandle> {
        self.task_manager_run_handle.take()
    }

    // TODO: remove allow once we actually use this function
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

    async fn get_state(&self) -> ConductorResult<ConductorState> {
        let guard = self.env.guard().await;
        let reader = guard.reader()?;
        Ok(self.state_db.get(&reader, &UnitDbKey)?.unwrap_or_default())
    }

    async fn spawn_admin_interface<Api: AdminInterfaceApi>(
        &mut self,
        _api: Api,
    ) -> ConductorResult<()> {
        self.check_running()?;
        unimplemented!()
    }

    async fn spawn_app_interface<Api: AppInterfaceApi>(
        &mut self,
        _api: Api,
    ) -> ConductorResult<()> {
        self.check_running()?;
        unimplemented!()
    }
}

type ConductorStateDb = Kv<UnitDbKey, ConductorState>;

mod builder {

    use super::*;
    use crate::conductor::{
        api::StdAdminInterfaceApi,
        config::AdminInterfaceConfig,
        interface::{websocket::spawn_admin_interface_task, InterfaceDriver},
        manager::keep_alive_task,
    };
    use futures::future;
    use std::sync::Arc;
    use sx_state::{env::EnvironmentKind, test_utils::test_conductor_env};
    use tokio::sync::RwLock;

    pub struct ConductorBuilder {}

    impl ConductorBuilder {
        pub fn new() -> Self {
            Self {}
        }

        pub async fn from_config(
            self,
            config: ConductorConfig,
        ) -> ConductorResult<ConductorHandle> {
            let env_path = config.environment_path;
            let environment = Environment::new(env_path.as_ref(), EnvironmentKind::Conductor)?;
            let conductor = Conductor::new(environment).await?;
            let stop_tx = conductor.managed_task_stop_sender.clone();
            let conductor_mutex = Arc::new(RwLock::new(conductor));

            setup_admin_interfaces_from_config(
                conductor_mutex.clone(),
                stop_tx,
                config.admin_interfaces.unwrap_or_default(),
            )
            .await?;

            Ok(conductor_mutex.into())
        }

        pub async fn test(self) -> ConductorResult<Conductor> {
            let environment = test_conductor_env();
            Conductor::new(environment).await
        }
    }

    /// Spawn all admin interface tasks, register them with the TaskManager,
    /// and modify the conductor accordingly, based on the config passed in
    async fn setup_admin_interfaces_from_config(
        conductor_mutex: Arc<RwLock<Conductor>>,
        stop_tx: StopBroadcaster,
        configs: Vec<AdminInterfaceConfig>,
    ) -> ConductorResult<()> {
        let admin_api = StdAdminInterfaceApi::new(conductor_mutex.clone().into());

        // Closure to process each admin config item
        let spawn_from_config = |AdminInterfaceConfig { driver, .. }| match driver {
            InterfaceDriver::Websocket { port } => {
                spawn_admin_interface_task(port, admin_api.clone(), stop_tx.subscribe())
            }
        };

        // spawn interface tasks, collect their JoinHandles,
        // and throw away the errors after logging them
        let handles: Vec<_> = future::join_all(configs.into_iter().map(spawn_from_config))
            .await
            .into_iter()
            // Log errors
            .inspect(|interface| {
                if let Err(ref e) = interface {
                    error!(error = e as &dyn Error, "Admin interface failed to parse");
                }
            })
            // Throw away errors
            .filter_map(Result::ok)
            .collect();

        {
            let mut conductor = conductor_mutex.write().await;

            // First, register the keepalive task, to ensure the conductor doesn't shut down
            // in the absence of other "real" tasks
            conductor
                .manage_task(ManagedTaskAdd::dont_handle(tokio::spawn(keep_alive_task(
                    stop_tx.subscribe(),
                ))))
                .await?;

            // Now that tasks are spawned, register them with the TaskManager
            for handle in handles {
                conductor
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
        }
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {

    use super::{Conductor, ConductorState};
    use crate::conductor::state::CellConfig;

    #[tokio::test]
    async fn can_update_state() {
        let conductor = Conductor::build().test().await.unwrap();
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
