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
use futures::future;
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
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
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

pub type StopSender = tokio::sync::oneshot::Sender<()>;
pub type StopReceiver = tokio::sync::oneshot::Receiver<()>;

#[derive(Clone, From, AsRef, Deref)]
pub struct ConductorHandle(Arc<RwLock<Conductor>>);

impl ConductorHandle {
    /// End all tasks run by the Conductor.
    pub async fn shutdown(self) {
        let mut conductor = self.0.write().await;
        conductor.shutdown().await;
    }
}

/// A Conductor is a group of [Cell]s
pub struct Conductor {
    // tx_network: NetSender,
    cells: HashMap<CellId, CellItem>,
    env: Environment,
    state_db: ConductorStateDb,
    closing: bool,
    _handle_map: HashMap<CellHandle, CellId>,
    _agent_keys: HashMap<AgentId, Keystore>,
    admin_interfaces: Vec<AdminInterfaceHandle>,
}

impl Conductor {
    async fn new(env: Environment) -> ConductorResult<Conductor> {
        let db: SingleStore = *env.dbs().await?.get(&db::CONDUCTOR_STATE)?;
        Ok(Conductor {
            env,
            state_db: Kv::new(db)?,
            cells: HashMap::new(),
            _handle_map: HashMap::new(),
            _agent_keys: HashMap::new(),
            admin_interfaces: Vec::new(),
            closing: false,
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

    fn check_running(&self) -> ConductorResult<()> {
        if self.closing {
            Err(ConductorError::ShuttingDown)
        } else {
            Ok(())
        }
    }

    async fn shutdown(&mut self) -> () {
        future::join_all(self.admin_interfaces.iter_mut().map(|i| i.stop())).await;
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
        interface::{websocket::create_admin_interface, InterfaceDriver},
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
            let conductor_mutex = Arc::new(RwLock::new(Conductor::new(environment).await?));
            let admin_api = StdAdminInterfaceApi::new(conductor_mutex.clone().into());
            let interface_futures: Vec<_> = config
                .admin_interfaces
                .unwrap_or_else(|| Vec::new())
                .into_iter()
                .map(|AdminInterfaceConfig { driver, .. }| match driver {
                    InterfaceDriver::Websocket { port } => {
                        create_admin_interface(admin_api.clone(), port)
                    }
                })
                .collect();
            let interfaces: Vec<AdminInterfaceHandle> = future::join_all(interface_futures)
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
                .map(AdminInterfaceHandle)
                .collect();
            {
                let mut conductor = conductor_mutex.write().await;
                conductor.admin_interfaces = interfaces;
            }
            Ok(conductor_mutex.into())
        }

        pub async fn test(self) -> ConductorResult<Conductor> {
            let environment = test_conductor_env();
            Conductor::new(environment).await
        }
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
