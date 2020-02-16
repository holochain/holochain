use super::autonomic::AutonomicProcess;
use crate::{
    agent::SourceChain,
    cell::error::{CellError, CellResult},
    nucleus::{ZomeInvocation, ZomeInvocationResult},
    txn::{dht::DhtPersistence, source_chain, source_chain::SourceChainPersistence},
    workflow, conductor_api::ConductorCellApiT,
};
use async_trait::async_trait;
use holochain_persistence_api::txn::CursorProvider;
use std::{
    hash::{Hash, Hasher},
    path::Path,
};
use sx_types::{
    agent::AgentId,
    dna::Dna,
    error::{SkunkError, SkunkResult},
    prelude::*,
    shims::*,
};

/// TODO: consider a newtype for this
pub type DnaAddress = Address;

/// The unique identifier for a running Cell.
/// Cells are uniquely determined by this pair - this pair is necessary
/// and sufficient to refer to a cell in a conductor
pub type CellId = (DnaAddress, AgentId);


impl<Api: ConductorCellApiT> Hash for Cell<Api> {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        (self.dna_address(), self.agent_id()).hash(state);
    }
}

impl<Api: ConductorCellApiT> PartialEq for Cell<Api> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[derive(Clone)]
pub struct Cell<Api: ConductorCellApiT> {
    id: CellId,
    chain_persistence: SourceChainPersistence,
    dht_persistence: DhtPersistence,
    conductor_api: Api,
}

impl<Api: ConductorCellApiT> Cell<Api> {
    fn dna_address(&self) -> &DnaAddress {
        &self.id.0
    }

    fn agent_id(&self) -> &AgentId {
        &self.id.1
    }

    fn source_chain(&self) -> SourceChain {
        SourceChain::new(&self.chain_persistence)
    }

    pub async fn invoke_zome(&self, invocation: ZomeInvocation) -> CellResult<ZomeInvocationResult> {
        let source_chain = SourceChain::new(&self.chain_persistence);
        let cursor_rw = self
            .chain_persistence
            .create_cursor_rw()
            .map_err(SkunkError::from)?;
        Ok(workflow::invoke_zome(invocation, source_chain).await?)
    }

    pub async fn handle_network_message(
        &self,
        msg: Lib3hToClient,
    ) -> CellResult<Option<Lib3hToClientResponse>> {
        Ok(workflow::handle_network_message(msg).await?)
    }

    pub async fn handle_autonomic_process(&self, process: AutonomicProcess) -> CellResult<()> {
        match process {
            AutonomicProcess::SlowHeal => unimplemented!(),
            AutonomicProcess::HealthCheck => unimplemented!(),
        }
    }
}

// impl<Api: ConductorCellApiT> Cell<Api> {
//     /// Checks if Cell has been initialized already
//     pub fn from_id(id: CellId) -> CellResult<Self> {
//         let chain_persistence = SourceChainPersistence::new(id.clone());
//         let dht_persistence = DhtPersistence::new(id.clone());
//         SourceChain::new(&chain_persistence).validate()?;
//         Ok(Cell {
//             id,
//             chain_persistence,
//             dht_persistence,
//         })
//     }

//     pub fn from_dna(agent_id: AgentId, dna: Dna) -> SkunkResult<Self> {
//         unimplemented!()
//     }
// }

pub struct CellBuilder<Api: ConductorCellApiT> {
    id: CellId,
    chain_persistence: Option<SourceChainPersistence>,
    dht_persistence: Option<DhtPersistence>,
    conductor_api: Api,
}

impl<Api: ConductorCellApiT> CellBuilder<Api> {
    pub fn new(id: CellId, conductor_api: Api) -> Self {
        Self {
            id,
            chain_persistence: None,
            dht_persistence: None,
            conductor_api
        }
    }

    pub fn with_dna(self, dna: Dna) -> Self {
        unimplemented!()
    }

    #[cfg(test)]
    pub fn with_test_persistence(mut self, dir: &Path) -> Self {
        self.chain_persistence = Some(SourceChainPersistence::test(&dir.join("chain")));
        self.dht_persistence = Some(DhtPersistence::test(&dir.join("dht")));
        self
    }

    pub fn build(self) -> Cell<Api> {
        let id = self.id.clone();
        Cell {
            id: self.id,
            chain_persistence: self
                .chain_persistence
                .unwrap_or_else(|| SourceChainPersistence::new(id.clone())),
            dht_persistence: self
                .dht_persistence
                .unwrap_or_else(|| DhtPersistence::new(id.clone())),
            conductor_api: self.conductor_api,
        }
    }
}

// These are possibly composable traits that describe how to get a resource,
// so instead of explicitly building resources, we can downcast a Cell to exactly
// the right set of resource getter traits
trait NetSend {
    fn network_send(&self, msg: Lib3hClientProtocol) -> SkunkResult<()>;
}

/// Simplification of holochain_net::connection::NetSend
/// Could use the trait instead, but we will want an impl of it
/// for just a basic crossbeam_channel::Sender, so I'm simplifying
/// to avoid making a change to holochain_net
pub type NetSender = futures::channel::mpsc::Sender<Lib3hClientProtocol>;

#[cfg(test)]
pub mod tests {

    use super::*;
    use crate::test_utils::fake_cell_id;
    use crate::conductor_api::MockConductorCellApi;

    #[test]
    fn can_create_cell() {
        let tmpdir = tempdir::TempDir::new("skunkworx").unwrap();
        let cell: Cell<MockConductorCellApi> = CellBuilder::new(fake_cell_id("a"), MockConductorCellApi::new())
            .with_test_persistence(tmpdir.path())
            .build();
    }
}
