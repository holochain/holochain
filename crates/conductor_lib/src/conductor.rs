use crate::config::Config;
use crate::{
    error::{ConductorError, ConductorResult},
    api::{self, ConductorApiExternal, ConductorApiInternal},
};
use async_trait::async_trait;
use crossbeam_channel::Receiver;
use futures::executor::ThreadPool;
use futures::task::Spawn;
use holochain_json_api::json::JsonString;
use lib3h_protocol::protocol_client::Lib3hClientProtocol;
use lib3h_protocol::protocol_server::Lib3hServerProtocol;
use skunkworx_core::types::ZomeInvocationResult;
use skunkworx_core::{
    cell::{CellApi, CellId},
    types::ZomeInvocation,
};
use skunkworx_core_types::error::SkunkResult;
use std::collections::{HashMap, HashSet};

/// A conductor-specific name for a Cell
/// (Used to be instance_id)
pub type CellHandle = String;
type Executor = ThreadPool;

pub struct CellState {
    /// Whether or not we should call any methods on the cell
    active: bool,
}

type NetReceive = Receiver<Lib3hServerProtocol>;

pub struct Conductor<Cell: CellApi> {
    cells: HashMap<Cell, CellState>,
    handle_map: HashMap<CellHandle, Cell>,
    executor: Executor,
}

impl<Cell: CellApi> Conductor<Cell> {
    pub fn new(
        executor: Executor,
    ) -> Self {
        Self {
            cells: HashMap::new(),
            handle_map: HashMap::new(),
            executor,
        }
    }

    pub async fn blah(&self) {

    }

    pub async fn invoke_zome(&self, cell: Cell, invocation: ZomeInvocation) -> ConductorResult<()> {
        unimplemented!()
    }

    // fn build_cell(cell_id: CellId) -> Cell {
    //     CellBuilder {
    //         cell_id,
    //         tx_network,
    //         tx_signal,
    //         tx_zome,
    //     }.into()
    // }
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

pub use builder::*;
