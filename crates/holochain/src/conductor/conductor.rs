//! A Conductor is a dynamically changing group of [Cell]s.
//!
//! A Conductor can be managed:
//! - externally, via a [ExternalConductorApi]
//! - from within a [Cell], via [CellConductorApi]
//!
//! In normal use cases, a single Holochain user runs a single Conductor in a single process.
//! However, there's no reason we can't have multiple Conductors in a single process, simulating multiple
//! users in a testing environment.

use crate::conductor::{
    api::error::{ConductorApiError, ConductorApiResult},
    cell::{Cell, NetSender},
    config::ConductorConfig,
    error::ConductorResult,
};
pub use builder::*;
use std::collections::HashMap;
use sx_types::{
    cell::{CellHandle, CellId},
    shims::Keystore,
};
use derive_more::AsRef;
use sx_types::agent::AgentId;
use sx_state::{prelude::{Reader, WriteManager}, db, env::{ReadManager, Environment}, exports::SingleStore, typed::{UnitDbKey, Kv}};
use super::state::ConductorState;

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

/// A Conductor is a group of [Cell]s
pub struct Conductor {
    // tx_network: NetSender,
    cells: HashMap<CellId, CellItem>,
    env: Environment,
    state_db: ConductorStateDb,
    _handle_map: HashMap<CellHandle, CellId>,
    _agent_keys: HashMap<AgentId, Keystore>,
}

impl Conductor {

    async fn new(env: Environment) -> ConductorResult<Conductor> {
        let db: SingleStore = env.dbs().await?.get(&db::CONDUCTOR_STATE)?.clone();
        Ok(Conductor {
            env,
            state_db: Kv::new(db)?,
            cells: HashMap::new(),
            _handle_map: HashMap::new(),
            _agent_keys: HashMap::new(),
        })
    }

    pub fn build() -> ConductorBuilder {
        ConductorBuilder::new()
    }

    pub fn cell_by_id(&self, cell_id: &CellId) -> ConductorApiResult<&Cell> {
        let item = self
            .cells
            .get(cell_id)
            .ok_or_else(|| ConductorApiError::CellMissing(cell_id.clone()))?;
        Ok(&item.cell)
    }

    pub fn tx_network(&self) -> &NetSender {
        unimplemented!()
    }

    async fn update_state<F: Send>(&self, f: F) -> ConductorResult<ConductorState>
    where
        F: FnOnce(ConductorState) -> ConductorResult<ConductorState>,
    {
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

    async fn get_state_db(&self) -> ConductorResult<ConductorStateDb> {
        let db: SingleStore = self.env.dbs().await?.get(&db::CONDUCTOR_STATE)?.clone();
        Ok(Kv::new(db)?)
    }
}

type ConductorStateDb = Kv<UnitDbKey, ConductorState>;

mod builder {

    use super::*;
    use sx_state::{test_utils::test_conductor_env, env::EnvironmentKind};

    pub struct ConductorBuilder {

    }

    impl ConductorBuilder {
        pub fn new() -> Self {
            Self { }
        }

        pub async fn from_config(self, config: ConductorConfig) -> ConductorResult<Conductor> {
            let env_path = config.environment_path;
            let environment = Environment::new(env_path.as_ref(), EnvironmentKind::Conductor)?;
            let conductor = Conductor::new(environment).await?;
            Ok(conductor)
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

        conductor.update_state(|mut state| {
            state.cells.push(cell_config.clone());
            Ok(state)
        }).await.unwrap();
        let state = conductor.get_state().await.unwrap();
        assert_eq!(state.cells, [cell_config]);
    }
}
