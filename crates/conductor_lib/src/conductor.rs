use crate::config::Config;
use crate::{
    api,
    error::{ConductorError, ConductorResult},
};
use crossbeam_channel::Receiver;
use futures::executor::ThreadPool;
use futures::task::Spawn;
use lib3h_protocol::protocol_server::Lib3hServerProtocol;
use skunkworx_core::{
    cell::{Cell, CellId},
    types::ZomeInvocation,
};
use std::collections::HashMap;

/// A conductor-specific name for a Cell
/// (Used to be instance_id)
pub type CellHandle = String;

pub struct CellState {
    /// Whether or not we should call any methods on the cell
    active: bool,
}

type NetReceive = Receiver<Lib3hServerProtocol>;

pub struct Conductor<E: Spawn> {
    cells: HashMap<CellHandle, CellState>,
    executor: E,
    rx_api: Receiver<api::ConductorApi>,
    rx_net: Receiver<NetReceive>,
}

impl<E: Spawn> Conductor<E> {
    pub fn new(
        executor: E,
        rx_api: Receiver<api::ConductorApi>,
        rx_net: Receiver<NetReceive>,
    ) -> Self {
        Self {
            cells: HashMap::new(),
            executor,
            rx_api,
            rx_net,
        }
    }

    async fn handle_api_message(&mut self, msg: api::ConductorApi) -> ConductorResult<()> {
        match msg {
            api::ConductorApi::ZomeInvocation(handle, invocation) => {
                let state = self
                    .cells
                    .get(&handle)
                    .ok_or_else(|| ConductorError::NoSuchCell(handle))?;
                if state.active {
                    Cell::new(cell_id).invoke_zome(invocation);
                    Ok(())
                } else {
                    Err(ConductorError::CellNotActive)
                }
            }
            api::ConductorApi::Admin(msg) => match msg {},
            api::ConductorApi::Crypto(msg) => match msg {
                api::Crypto::Sign(payload) => unimplemented!(),
                api::Crypto::Encrypt(payload) => unimplemented!(),
                api::Crypto::Decrypt(payload) => unimplemented!(),
            },
            api::ConductorApi::Test(msg) => match msg {
                api::Test::AddAgent(args) => unimplemented!(),
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
