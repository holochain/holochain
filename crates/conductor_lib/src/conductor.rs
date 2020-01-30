use crate::config::DnaLoader;
use std::collections::HashMap;
use std::sync::Arc;
use sx_core::cell::CellApi;
use sx_core::cell::CellId;
use sx_core::cell::NetSender;
use sx_types::shims::*;

/// A conductor-specific name for a Cell
/// (Used to be instance_id)
pub type CellHandle = String;

/// Conductor-specific Cell state, this can probably be stored in a database.
/// Hypothesis: If nothing remains in this struct, then the Conductor state is
/// essentially immutable, and perhaps we just throw it out and make a new one
/// when we need to load new config, etc.
pub struct CellState<Cell: CellApi> {
    /// Whether or not we should call any methods on the cell
    active: bool,
    cell: Cell,
}

pub struct Conductor<Cell: CellApi> {
    tx_network: NetSender,
    cells: HashMap<CellId, CellState<Cell>>,
    handle_map: HashMap<CellHandle, Cell>,
    dna_loader: DnaLoader,
    // passphrase_manager: PassphraseManager,
    // key_loader: KeyLoader,
}

impl<Cell: CellApi> Conductor<Cell> {
    pub fn new(tx_network: NetSender) -> Self {
        Self {
            cells: HashMap::new(),
            handle_map: HashMap::new(),
            tx_network,
            dna_loader: Arc::new(Box::new(|_| unimplemented!())),
        }
    }

    pub fn tx_network(&self) -> &NetSender {
        &self.tx_network
    }
}

mod builder {

    use super::*;
    use crate::config::Config;
    use crate::error::ConductorResult;
    use sx_core::cell::Cell;

    pub struct ConductorBuilder {
        // executor: Option<Box<dyn Spawn>>,
    }

    impl ConductorBuilder {
        pub fn new() -> Self {
            Self {}
        }

        // pub fn executor(mut self, executor: Box<dyn Spawn>) -> Self {
        //     self.executor = Some(Box::new(executor));
        //     self
        // }

        pub fn from_config(self, config: Config) -> ConductorResult<Conductor<Cell>> {
            unimplemented!()
            // let executor = self.executor.unwrap_or_else(default_executor);

            // Ok(Conductor {
            //     cells: HashMap::new(),
            //     // executor,
            // })
        }
    }

    // fn default_executor() -> Box<dyn Spawn> {
    //     Box::new(ThreadPool::new().expect("Couldn't create Threadpool executor"))
    // }
}

pub use builder::*;
