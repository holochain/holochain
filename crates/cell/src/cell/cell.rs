use crate::{
    cell::error::{CellResult},
    ribosome::Ribosome,
};
use async_trait::async_trait;
use std::hash::{Hash, Hasher};
use sx_state::{
    env::{Environment},
};
use sx_types::{
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
    agent::AgentId,
    error::{SkunkResult},
    shims::*,
    autonomic::AutonomicProcess
};
use sx_conductor_api::error::ConductorApiResult;
use sx_conductor_api::interface::CellConductorInterfaceT;
use sx_conductor_api::cell::CellT;

/// TODO: consider a newtype for this
pub type DnaAddress = sx_types::dna::DnaAddress;

/// The unique identifier for a running Cell.
/// Cells are uniquely determined by this pair - this pair is necessary
/// and sufficient to refer to a cell in a conductor
pub type CellId = sx_types::agent::CellId;


impl<I: CellConductorInterfaceT> Hash for Cell<I> {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        (self.dna_address(), self.agent_id()).hash(state);
    }
}

impl<I: CellConductorInterfaceT> PartialEq for Cell<I> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[async_trait]
impl<I: CellConductorInterfaceT> CellT for Cell<I> {
    type Interface = I;

    async fn invoke_zome(
        &self,
        _conductor_api: I,
        _invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse> {
        unimplemented!()
    }

}


pub struct Cell<I: CellConductorInterfaceT> {
    id: CellId,
    state_env: Environment,
    _conductor_api: I,
}

impl<I: CellConductorInterfaceT> Cell<I> {
    fn dna_address(&self) -> &DnaAddress {
        &self.id.dna_address()
    }

    fn agent_id(&self) -> &AgentId {
        &self.id.agent_id()
    }

    pub(crate) fn get_ribosome(&self) -> Ribosome {
        unimplemented!()
    }

    pub(crate) fn state_env(&self) -> Environment {
        self.state_env.clone()
    }

    pub async fn handle_network_message(
        &self,
        _msg: Lib3hToClient,
    ) -> CellResult<Option<Lib3hToClientResponse>> {
        unimplemented!()
    }

    pub async fn handle_autonomic_process(&self, process: AutonomicProcess) -> CellResult<()> {
        match process {
            AutonomicProcess::SlowHeal => unimplemented!(),
            AutonomicProcess::HealthCheck => unimplemented!(),
        }
    }
}

// impl<I: CellConductorInterfaceT> Cell<I> {
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

// pub struct CellBuilder<I: CellConductorInterfaceT> {
//     id: CellId,
//     chain_persistence: Option<SourceChainPersistence>,
//     dht_persistence: Option<DhtPersistence>,
//     conductor_api: I,
// }

// impl<I: CellConductorInterfaceT> CellBuilder<I> {
//     pub fn new(id: CellId, conductor_api: I) -> Self {
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

//     pub fn build(self) -> Cell<I> {
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
//             db_manager: DbManager::new(create_lmdb_env(DatabasePath::from(id).as_ref())),
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

    // #[test]
    // fn can_create_cell() {
    //     let tmpdir = tempdir::TempDir::new("skunkworx").unwrap();
    //     let cell: Cell<MockConductorCellApi> =
    //         CellBuilder::new(fake_cell_id("a"), MockConductorCellApi::new())
    //             .with_test_persistence(tmpdir.path())
    //             .build();
    // }
}
