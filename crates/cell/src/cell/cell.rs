use super::autonomic::AutonomicProcess;
use crate::{
    agent::SourceChain,
    cell::error::{CellError, CellResult},
    conductor_api::ConductorCellApiT,
    nucleus::{ZomeInvocation, ZomeInvocationResult},
    ribosome::Ribosome,
    txn::{dht::DhtPersistence, source_chain, source_chain::SourceChainPersistence},
    workflow,
};
use async_trait::async_trait;
use holochain_persistence_api::txn::CursorProvider;
use std::{
    hash::{Hash, Hasher},
    path::Path, sync::{Arc, RwLock},
};
use sx_types::{
    agent::AgentId,
    dna::Dna,
    error::{SkunkError, SkunkResult},
    prelude::*,
    shims::*,
};
use sx_state::RkvEnv;

/// TODO: consider a newtype for this
pub type DnaAddress = Address;

/// The unique identifier for a running Cell.
/// Cells are uniquely determined by this pair - this pair is necessary
/// and sufficient to refer to a cell in a conductor
pub type CellId = (DnaAddress, AgentId);
pub type ZomeId = (CellId, ZomeName);
pub type ZomeName = String;

impl<'env, Api: ConductorCellApiT> Hash for Cell<'env, Api> {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        (self.dna_address(), self.agent_id()).hash(state);
    }
}

impl<'env, Api: ConductorCellApiT> PartialEq for Cell<'env, Api> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[derive(Clone)]
pub struct Cell<'env, Api: ConductorCellApiT> {
    id: CellId,
    state_env: &'env RkvEnv,
    dht_persistence: DhtPersistence,
    conductor_api: Api,
}

impl<'env, Api: ConductorCellApiT> Cell<'env, Api> {
    fn dna_address(&self) -> &DnaAddress {
        &self.id.0
    }

    fn agent_id(&self) -> &AgentId {
        &self.id.1
    }

    fn source_chain(&self) -> SourceChain {
        SourceChain::new(&self.chain_persistence)
    }

    pub async fn invoke_zome(
        &self,
        conductor_api: Api,
        invocation: ZomeInvocation,
    ) -> CellResult<ZomeInvocationResult> {
        let source_chain = SourceChain::new(self.state_env);
        let writer = self.state_env.write()?;
        let previous_head = source_chain.head()?;
        let dna = source_chain.dna()?;
        let ribosome = Ribosome::new(dna);
        let invoke_result =
            workflow::invoke_zome(invocation, source_chain, ribosome, conductor_api).await?;
        workflow::publish(source_chain.now().unwrap(), &previous_head).await?;
        Ok(invoke_result)
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

// im'env, pl<Api: ConductorCellApiT> Cell<'env, Api> {
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

// pub struct CellBuilder<Api: ConductorCellApiT> {
//     id: CellId,
//     chain_persistence: Option<SourceChainPersistence>,
//     dht_persistence: Option<DhtPersistence>,
//     conductor_api: Api,
// }

// impl<Api: ConductorCellApiT> CellBuilder<Api> {
//     pub fn new(id: CellId, conductor_api: Api) -> Self {
//         Self {
//             id,
//             chain_persistence: None,
//             dht_persistence: None,
//             conductor_api,
//         }
//     }

//     pub fn with_dna(self, dna: Dna) -> Self {
//         unimplemented!()
//     }

//     #[cfg(test)]
//     pub fn with_test_persistence(mut self, dir: &Path) -> Self {
//         self.chain_persistence = Some(SourceChainPersistence::test(&dir.join("chain")));
//         self.dht_persistence = Some(DhtPersistence::test(&dir.join("dht")));
//         self
//     }

//   'env,   pub fn build(self) -> Cell<'env, Api> {
//         let id = self.id.clone();
//         Cell {
//             id: self.id,
//             chain_persistence: self
//                 .chain_persistence
//                 .unwrap_or_else(|| SourceChainPersistence::new(id.clone())),
//             dht_persistence: self
//                 .dht_persistence
//                 .unwrap_or_else(|| DhtPersistence::new(id.clone())),
//             conductor_api: self.conductor_api,
//         }
//     }
// }

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
    use crate::{conductor_api::MockConductorCellApi, test_utils::fake_cell_id};

    // #[test]
    // fn can_create_cell() {
    //     let tmpdir = tempdir::TempDir::new("skunkworx").unwrap();
    //     let cell: Cell<MockConductorCellApi> =
    //         CellBuilder::new(fake_cell_id("a"), MockConductorCellApi::new())
    //             .with_test_persistence(tmpdir.path())
    //             .build();
    // }
}
