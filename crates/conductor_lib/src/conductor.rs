use crate::config::Config;
use async_trait::async_trait;
use crate::{
    protocol::{self, ConductorApiInternal, ConductorApiExternal},
    error::{ConductorError, ConductorResult},
};
use crossbeam_channel::Receiver;
use futures::executor::ThreadPool;
use futures::task::Spawn;
use lib3h_protocol::protocol_server::Lib3hServerProtocol;
use skunkworx_core::{
    cell::{CellApi, CellId},
    types::ZomeInvocation,
};
use std::collections::{HashMap, HashSet};
use skunkworx_core::types::ZomeInvocationResult;
use skunkworx_core_types::error::SkunkResult;

/// A conductor-specific name for a Cell
/// (Used to be instance_id)
pub type CellHandle = String;

pub struct CellState {
    /// Whether or not we should call any methods on the cell
    active: bool,
}

type NetReceive = Receiver<Lib3hServerProtocol>;

pub struct Conductor<Cell: CellApi, E: Spawn> {
    cells: HashMap<Cell, CellState>,
    handle_map: HashMap<CellHandle, Cell>,
    executor: E,
    rx_api: Receiver<protocol::ConductorProtocol>,
    rx_net: Receiver<NetReceive>,
}

#[async_trait]
impl<Cell: CellApi, E: Spawn> ConductorApiInternal<Cell> for Conductor<Cell, E> {
    async fn invoke_zome(cell: Cell, invocation: ZomeInvocation) -> ZomeInvocationResult {
        unimplemented!()
    }

    async fn net_send(message: Lib3hClientProtocol) -> SkunkResult<()> {
        unimplemented!()
    }

    async fn net_request(message: Lib3hClientProtocol) -> SkunkResult<Lib3hServerProtocol> {
        unimplemented!()
    }
}

impl<Cell: CellApi, E: Spawn> Conductor<Cell, E> {
    pub fn new(
        executor: E,
        rx_api: Receiver<protocol::ConductorProtocol>,
        rx_net: Receiver<NetReceive>,
    ) -> Self {
        Self {
            cells: HashMap::new(),
            handle_map: HashMap::new(),
            executor,
            rx_api,
            rx_net,
        }
    }

    pub async fn handle_message(&mut self, msg: protocol::ConductorProtocol) -> ConductorResult<()> {
        match msg {
            protocol::ConductorProtocol::ZomeInvocation(handle, invocation) => {
                let cell = self
                    .handle_map
                    .get(&handle)
                    .ok_or_else(|| ConductorError::NoSuchCell(handle.clone()))?;
                let state = self
                    .cells
                    .get(&cell)
                    .ok_or_else(|| ConductorError::NoSuchCell(handle))?;
                if state.active {
                    cell.invoke_zome(invocation);
                    Ok(())
                } else {
                    Err(ConductorError::CellNotActive)
                }
            }
            protocol::ConductorProtocol::Admin(msg) => match msg {},
            protocol::ConductorProtocol::Crypto(msg) => match msg {
                protocol::Crypto::Sign(payload) => unimplemented!(),
                protocol::Crypto::Encrypt(payload) => unimplemented!(),
                protocol::Crypto::Decrypt(payload) => unimplemented!(),
            },
            protocol::ConductorProtocol::Network(msg) => match msg {
                _ => unimplemented!()
            },
            protocol::ConductorProtocol::Test(msg) => match msg {
                protocol::Test::AddAgent(args) => unimplemented!(),
            },
        }
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
