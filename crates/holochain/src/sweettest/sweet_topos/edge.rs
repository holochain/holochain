use super::network::NetworkTopologyConductor;
use super::node::NetworkTopologyNode;
use crate::sweettest::SweetConductor;
use arbitrary::Arbitrary;
use holochain_zome_types::prelude::CellId;
use rand::Rng;
use std::hash::{Hash, Hasher};

/// A network edge in a network topology. Represents a network connection.
/// Edges are directed, so if you want a bidirectional connection you need two
/// edges.
#[derive(Arbitrary, Clone, Debug, Default, Eq)]
pub struct NetworkTopologyEdge {
    id: [u8; 32],
    source_conductor: NetworkTopologyConductor,
    target_conductor: NetworkTopologyConductor,
    cells: Vec<CellId>,
}

/// ID based hashing means we can use edges as keys in a hashmap and they'll
/// be treated as duplicate even if mutated.
impl Hash for NetworkTopologyEdge {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

/// ID based equality is good for topology edges so we can track them
/// independently no matter what kind of mutations/state might eventuate.
impl PartialEq for NetworkTopologyEdge {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl NetworkTopologyEdge {
    /// Get the cells in this edge.
    /// MAY disagree with the cells in the conductor.
    /// This is because the conductor may have been mutated since the edge was
    /// created or vice versa re: apply.
    pub fn cells(&self) -> Vec<CellId> {
        self.cells.clone()
    }

    /// Get the source conductor for this edge.
    pub fn source_conductor(&self) -> &NetworkTopologyConductor {
        &self.source_conductor
    }

    /// Get the target conductor for this edge.
    pub fn target_conductor(&self) -> &NetworkTopologyConductor {
        &self.target_conductor
    }

    /// Apply the edge state to its associated conductor.
    pub async fn apply(&mut self) -> anyhow::Result<()> {
        async fn envs_from_conductor(
            conductor: &tokio::sync::RwLock<SweetConductor>,
        ) -> Vec<holochain_types::db::DbWrite<holochain_types::db::DbKindP2pAgents>> {
            let mut envs = Vec::new();

            for env in conductor
                .read()
                .await
                .raw_handle()
                .spaces
                .get_from_spaces(|s| s.p2p_agents_db.clone())
            {
                envs.push(env.clone());
            }
            envs
        }

        let source_envs = envs_from_conductor(self.source_conductor().lock().await).await;
        let target_envs = envs_from_conductor(self.target_conductor().lock().await).await;

        // @todo This reveals all peers to the source, but in reality we'd only
        // want to reveal the list of agents specified by the edge.
        crate::conductor::p2p_agent_store::reveal_peer_info(source_envs, target_envs).await;

        Ok(())
    }

    /// Create a new edge with a full view on the given target node.
    pub fn new_full_view_on_node(
        source: &NetworkTopologyNode,
        target: &NetworkTopologyNode,
    ) -> Self {
        let mut rng = rand::thread_rng();
        Self {
            id: rng.gen(),
            source_conductor: source.conductor().clone(),
            target_conductor: target.conductor().clone(),
            cells: target.cells(),
        }
    }
}
