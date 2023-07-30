use crate::sweettest::SweetAgents;
use crate::sweettest::SweetConductor;
use arbitrary::Arbitrary;
use arbitrary::Unstructured;
use async_once_cell::OnceCell;
use holo_hash::HasHash;
use holochain_types::prelude::AgentPubKey;
use holochain_types::prelude::DnaFile;
use holochain_types::share::RwShare;
use holochain_util::tokio_helper;
use holochain_zome_types::prelude::CellId;
use rand::Rng;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

/// Some orphan rule hoop jumping.
#[derive(Clone, Debug)]
struct NetworkTopologyNodeConductor(Arc<OnceCell<RwShare<SweetConductor>>>);

impl NetworkTopologyNodeConductor {
    pub async fn get_share(&self) -> &RwShare<SweetConductor> {
        self.0
            .get_or_init(async { RwShare::new(SweetConductor::from_standard_config().await) })
            .await
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
impl<'a> Arbitrary<'a> for NetworkTopologyNodeConductor {
    fn arbitrary(_u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
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
        let conductor_share = self.conductor.get_share().await;
        let keystore = conductor_share.share_ref(|conductor| conductor.keystore());
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

        let conductor_share = self.conductor.get_share().await;
        let conductor_cells = conductor_share.share_ref(|conductor| {
            conductor
                .live_cell_ids()
                .iter()
                .cloned()
                .collect::<HashSet<_>>()
        });

        for (dna_file, keys) in &self.agents {
            for key in keys {
                let cell_id = CellId::new(dna_file.dna().as_hash().to_owned(), key.clone());
                if !conductor_cells.contains(&cell_id) {
                    conductor_share.share_mut(|conductor| {
                        tokio_helper::block_forever_on(async move {
                            conductor
                                .setup_app_for_agent("app-", key.clone(), [&dna_file.clone()])
                                .await
                        })
                    })?;
                }
            }
        }

        let cells_to_remove = conductor_cells
            .difference(&node_cells)
            .cloned()
            .collect::<Vec<_>>();
        conductor_share.share_mut(|conductor| {
            tokio_helper::block_forever_on(async move {
                conductor.raw_handle().remove_cells(&cells_to_remove).await;
            })
        });

        Ok(())
    }
}
