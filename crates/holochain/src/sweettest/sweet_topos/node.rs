use arbitrary::Arbitrary;
use holochain_zome_types::prelude::CellId;
use crate::sweettest::SweetConductor;
use std::collections::HashMap;
use rand::Rng;
use async_once_cell::OnceCell;
use arbitrary::Unstructured;
use holochain_types::prelude::AgentPubKey;
use std::sync::Arc;
use std::collections::HashSet;
use holochain_types::prelude::DnaFile;
use holo_hash::HasHash;
use crate::sweettest::SweetAgents;
use parking_lot::Mutex;

/// Some orphan rule hoop jumping.
#[derive(Clone, Debug)]
struct NetworkTopologyNodeConductor(OnceCell<SweetConductor>);

impl NetworkTopologyNodeConductor {
    pub async fn get(&self) -> &SweetConductor {
        self.0.get_or_init(async {
            SweetConductor::from_standard_config().await
        }).await
    }

    pub async fn get_mut(&mut self) -> &mut SweetConductor {
        // Ensure it is initialized.
        let _ = self.get().await;
        self.0.get_mut().unwrap()
    }

    pub fn new() -> Self {
        Self(Arc::new(OnceCell::new()))
    }
}

/// A node in a network topology. Represents a conductor.
#[derive(Arbitrary, Clone, Debug)]
pub struct NetworkTopologyNode {
    id: [u8; 32],
    agents: HashMap<DnaFile, HashSet<AgentPubKey>>,
    conductor: NetworkTopologyNodeConductor,
}

/// This implementation exists so that the parent NetworkTopologyNode can itself
/// implement Arbitrary. It creates an empty once cell which will be filled in
/// by `get` and then ultimately needs to have the parent node apply its state.
impl <'a> Arbitrary<'a> for NetworkTopologyNodeConductor {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(NetworkTopologyNodeConductor::new())
    }
}

/// ID based equality is good for topology nodes so we can track them
/// independently no matter what kind of mutations/state might eventuate.
impl PartialEq for NetworkTopologyNode {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl NetworkTopologyNode {

    /// Create a new node with a new conductor, and no cells.
    pub async fn new() -> Self {
        let mut rng = rand::thread_rng();
        Self {
            id: rng.gen(),
            agents: HashMap::new(),
            conductor: NetworkTopologyNodeConductor::new(),
        }
    }

    /// Get the cells in this node. MAY disagree with the cells in the conductor.
    pub fn cells(&self) -> Vec<CellId> {
        self.agents.iter().flat_map(|(dna_file, agents)| {
            agents.iter().map(|agent| {
                CellId::new(dna_file.dna().as_hash().to_owned(), agent.to_owned())
            })
        }).collect()
    }

    /// Ensure every given DnaFile is installed in this node. This is idempotent.
    pub fn ensure_dnas(&mut self, dnas: Vec<DnaFile>) {
        for dna in dnas {
            self.agents.entry(dna).or_default();
        }
    }

    /// Generate cells by generating agents under dna files. Currently only
    /// supports adding each generated agent to EVERY dna file.
    pub async fn generate_cells(&mut self, count: usize) {
        let agents = SweetAgents::get(self.conductor.get().await.keystore(), count).await;

        let dnas = self.agents.keys().cloned().collect::<Vec<_>>();
        for dna in dnas {
            self.agents.get_mut(&dna).unwrap().extend(agents.clone());
        }
    }

    /// Apply the state of the network node to its associated conductor. This is
    /// done by removing all cells from the conductor that are not in the node,
    /// then adding all remaining cells in the node to the conductor.
    pub async fn apply(&mut self) {
        let node_cells = self.cells().into_iter().collect::<HashSet<_>>();

        let conductor = self.conductor.get_mut().await;
        let conductor_cells: HashSet<CellId> = conductor.raw_handle().live_cell_ids().iter().cloned().collect();

        for (dna_file, keys) in &self.agents {
            for key in keys {
                let cell_id = CellId::new(dna_file.dna().as_hash().to_owned(), key.clone());
                if !conductor_cells.contains(&cell_id) {
                    conductor.setup_app_for_agent("app-", key.clone(), [dna_file]).await;
                }
            }
        }

        let cells_to_remove = conductor_cells.difference(&node_cells).cloned().collect::<Vec<_>>();
        conductor.raw_handle().remove_cells(&cells_to_remove.clone()).await;

    }
}
