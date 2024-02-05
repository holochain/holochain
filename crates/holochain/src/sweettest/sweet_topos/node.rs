use super::network::NetworkTopologyConductor;
use crate::sweettest::SweetAgents;
use arbitrary::Arbitrary;
use holo_hash::HasHash;
use holochain_types::prelude::AgentPubKey;
use holochain_types::prelude::DnaFile;
use holochain_zome_types::prelude::CellId;
use rand::Rng;
use std::collections::HashMap;
use std::collections::HashSet;

/// A node in a network topology. Represents a conductor.
#[derive(Arbitrary, Clone, Debug)]
pub struct NetworkTopologyNode {
    id: [u8; 32],
    agents: HashMap<DnaFile, HashSet<AgentPubKey>>,
    conductor: NetworkTopologyConductor,
}

/// ID based equality is good for topology nodes so we can track them
/// independently no matter what kind of mutations/state might eventuate.
impl PartialEq for NetworkTopologyNode {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Default for NetworkTopologyNode {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkTopologyNode {
    /// Create a new node with a new conductor, and no cells.
    pub fn new() -> Self {
        let mut rng = rand::thread_rng();
        Self {
            id: rng.gen(),
            agents: HashMap::new(),
            conductor: NetworkTopologyConductor::new(),
        }
    }

    /// Get the conductor for this node.
    pub fn conductor(&self) -> &NetworkTopologyConductor {
        &self.conductor
    }

    /// Get the cells in this node. MAY disagree with the cells in the conductor.
    pub fn cells(&self) -> Vec<CellId> {
        self.agents
            .iter()
            .flat_map(|(dna_file, agents)| {
                agents
                    .iter()
                    .map(|agent| CellId::new(dna_file.dna().as_hash().to_owned(), agent.to_owned()))
            })
            .collect()
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
        let keystore = self.conductor.lock().await.read().await.keystore();
        let agents = SweetAgents::get(keystore, count).await;

        let dnas = self.agents.keys().cloned().collect::<Vec<_>>();
        for dna in dnas {
            self.agents.get_mut(&dna).unwrap().extend(agents.clone());
        }
    }

    /// Apply the state of the network node to its associated conductor. This is
    /// done by removing all cells from the conductor that are not in the node,
    /// then adding all remaining cells in the node to the conductor.
    pub async fn apply(&mut self) -> anyhow::Result<()> {
        let node_cells = self.cells().into_iter().collect::<HashSet<_>>();

        let conductor_cells = self
            .conductor
            .lock()
            .await
            .read()
            .await
            .running_cell_ids()
            .iter()
            .cloned()
            .collect::<HashSet<_>>();

        for (dna_file, keys) in &self.agents {
            for key in keys {
                let cell_id = CellId::new(dna_file.dna().as_hash().to_owned(), key.clone());
                if !conductor_cells.contains(&cell_id) {
                    self.conductor
                        .lock()
                        .await
                        .write()
                        .await
                        .setup_app_for_agent(
                            &format!("{}", &cell_id),
                            key.clone(),
                            &[dna_file.clone()],
                        )
                        .await?;
                }
            }
        }

        let cells_to_remove = conductor_cells
            .difference(&node_cells)
            .cloned()
            .collect::<Vec<_>>();
        self.conductor
            .lock()
            .await
            .write()
            .await
            .raw_handle()
            .remove_cells(&cells_to_remove)
            .await;

        Ok(())
    }
}
