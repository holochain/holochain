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
// use sx_keystore::keystore::Keystore;
use sx_types::agent::AgentId;
use sx_state::env::Environment;

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
    _environment: Environment,
    _handle_map: HashMap<CellHandle, CellId>,
    _agent_keys: HashMap<AgentId, Keystore>,
}

impl Conductor {

    pub fn new() -> ConductorBuilder {
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

    pub fn load_config(_config: ConductorConfig) -> ConductorResult<()> {
        Ok(())
    }
}

mod builder {

    use super::*;
    use sx_state::{test_utils::test_conductor_env, env::EnvironmentKind};
    use crate::conductor::config::EnvironmentPath;

    pub struct ConductorBuilder {

    }

    impl ConductorBuilder {
        pub fn new() -> Self {
            Self { }
        }

        pub fn from_config(self, config: ConductorConfig) -> ConductorResult<Conductor> {
            let env_path: EnvironmentPath = config.environment_path.map(Into::into).unwrap_or_default();
            let environment = Environment::new(env_path.as_ref(), EnvironmentKind::Conductor)?;
            Ok(Conductor {
                cells: HashMap::new(),
                // tx_network,
                _environment: environment,
                _handle_map: HashMap::new(),
                _agent_keys: HashMap::new(),
            })
        }

        pub fn test(self) -> Conductor {
            let environment = test_conductor_env();
            Conductor {
                cells: HashMap::new(),
                // tx_network,
                _environment: environment.into(),
                _handle_map: HashMap::new(),
                _agent_keys: HashMap::new(),
            }
        }
    }
}
