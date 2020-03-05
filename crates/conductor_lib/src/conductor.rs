use std::collections::HashMap;
use sx_cell::{
    cell::{Cell, CellId, NetSender},
    conductor_api::ConductorCellApiT,
};
use sx_types::shims::Keystore;

/// A conductor-specific name for a Cell
/// (Used to be instance_id)
pub type CellHandle = String;

/// Conductor-specific Cell state, this can probably be stored in a database.
/// Hypothesis: If nothing remains in this struct, then the Conductor state is
/// essentially immutable, and perhaps we just throw it out and make a new one
/// when we need to load new config, etc.
pub struct CellState {
    /// Whether or not we should call any methods on the cell
    active: bool,
}

pub struct CellItem<Api: ConductorCellApiT> {
    cell: Cell<Api>,
    state: CellState,
}

pub struct Conductor<Api: ConductorCellApiT> {
    tx_network: NetSender,
    cells: HashMap<CellId, CellItem<Api>>,
    handle_map: HashMap<CellHandle, CellId>,
    agent_keys: HashMap<AgentId, Keystore>,
}

impl<Api: ConductorCellApiT> Conductor<Api> {
    pub fn new(tx_network: NetSender) -> Self {
        Self {
            cells: HashMap::new(),
            handle_map: HashMap::new(),
            tx_network,
            agent_keys: HashMap::new(),
        }
    }

    pub fn cell_by_id(&self, cell_id: &CellId) -> ConductorResult<&Cell<Api>> {
        let item = self
            .cells
            .get(cell_id)
            .ok_or_else(|| ConductorError::CellMissing(cell_id.clone()))?;
        Ok(&item.cell)
    }

    pub fn tx_network(&self) -> &NetSender {
        &self.tx_network
    }

    pub fn load_config(_config: Config) -> ConductorResult<()> {
        Ok(())
    }
}

mod builder {

    // use super::*;

    // pub struct ConductorBuilder {
    //     executor: Option<Box<dyn Spawn>>,
    // }

    // impl ConductorBuilder {
    //     pub fn new() -> Self {
    //         Self { executor: None }
    //     }

    //     pub fn executor(mut self, executor: Box<dyn Spawn>) -> Self {
    //         self.executor = Some(Box::new(executor));
    //         self
    //     }

    //     pub fn from_config(self, config: Config) -> ConductorResult<Conductor<Box<dyn Spawn>>> {
    //         let executor = self.executor.unwrap_or_else(default_executor);
    //         Ok(Conductor {
    //             cells: HashMap::new(),
    //             executor,
    //         })
    //     }
    // }

    // fn default_executor() -> Box<dyn Spawn> {
    //     Box::new(ThreadPool::new().expect("Couldn't create Threadpool executor"))
    // }
}

use crate::{
    config::Config,
    error::{ConductorError, ConductorResult},
};
pub use builder::*;
// use sx_keystore::keystore::Keystore;
use sx_types::agent::AgentId;
