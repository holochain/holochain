use skunkworx_core::cell::NetSender;
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

/// Conductor-specific Cell state, this can probably be stored in a database.
/// Hypothesis: If nothing remains in this struct, then the Conductor state is
/// essentially immutable, and perhaps we just throw it out and make a new one
/// when we need to load new config, etc.
pub struct CellState {
    /// Whether or not we should call any methods on the cell
    active: bool,
}

type NetReceive = Receiver<Lib3hServerProtocol>;

pub struct Conductor<Cell: CellApi> {
    tx_network: NetSender,
    cells: HashMap<Cell, CellState>,
    handle_map: HashMap<CellHandle, Cell>,
}

impl<Cell: CellApi> Conductor<Cell> {
    pub fn new(tx_network: NetSender) -> Self {
        Self {
            cells: HashMap::new(),
            handle_map: HashMap::new(),
            tx_network,
        }
    }

    pub fn tx_network(&self) -> &NetSender {
        &self.tx_network
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

pub use builder::*;
