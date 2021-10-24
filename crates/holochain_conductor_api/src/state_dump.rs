use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_state::source_chain::SourceChainJsonDump;
use holochain_types::dht_op::DhtOp;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
pub struct JsonDump {
    pub peer_dump: P2pAgentsDump,
    pub source_chain_dump: SourceChainJsonDump,
    pub integration_dump: IntegrationStateDump,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
/// A collection of many cells dumps for easy viewing.
/// Use display to see a nice printout.
pub struct IntegrationStateDumps(pub Vec<IntegrationStateDump>);

#[derive(Serialize, Deserialize, Debug, Clone)]
/// A high level view of the incoming ops and where
/// they are currently.
/// Ops start in the validation limbo then proceed
/// to the integration limbo then finally are integrated.
pub struct IntegrationStateDump {
    /// Ops in validation limbo awaiting sys
    /// or app validation.
    pub validation_limbo: Vec<DhtOp>,

    /// Ops waiting to be integrated.
    pub integration_limbo: Vec<DhtOp>,

    /// Ops that are integrated.
    /// This includes rejected.
    pub integrated: Vec<DhtOp>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
/// State dump of all the peer info
pub struct P2pAgentsDump {
    /// The info of this agents cell.
    pub this_agent_info: Option<AgentInfoDump>,
    /// The dna as a [`DnaHash`] and [`kitsune_p2p::KitsuneSpace`].
    pub this_dna: Option<(DnaHash, kitsune_p2p::KitsuneSpace)>,
    /// The agent as [`AgentPubKey`] and [`kitsune_p2p::KitsuneAgent`].
    pub this_agent: Option<(AgentPubKey, kitsune_p2p::KitsuneAgent)>,
    /// All other agent info.
    pub peers: Vec<AgentInfoDump>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
/// Agent info dump with the agent,
/// space, signed time, expires in and
/// urls printed in a pretty way.
pub struct AgentInfoDump {
    pub kitsune_agent: Arc<kitsune_p2p::KitsuneAgent>,
    pub kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
    pub dump: String,
}

impl std::fmt::Display for JsonDump {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let num_other_peers = self.peer_dump.peers.len();
        let s = &self.source_chain_dump;
        writeln!(f, "--- Cell State Dump Summary ---")?;
        writeln!(
            f,
            "Number of other peers in p2p store: {},",
            num_other_peers
        )?;
        writeln!(
            f,
            "Elements authored: {}, Ops published: {}",
            s.elements.len(),
            s.published_ops_count
        )
    }
}

impl std::fmt::Display for IntegrationStateDumps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        for i in &self.0 {
            write!(f, "{},", i)?;
        }
        writeln!(f, "]")
    }
}

impl std::fmt::Display for IntegrationStateDump {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "({:?},{:?},{:?})",
            self.validation_limbo, self.integration_limbo, self.integrated
        )
    }
}

impl std::fmt::Display for AgentInfoDump {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "space: {:?}", self.kitsune_space)?;
        writeln!(f, "agent: {:?}", self.kitsune_agent)?;
        writeln!(f, "{}", self.dump)
    }
}
impl std::fmt::Display for P2pAgentsDump {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(this_agent) = &self.this_agent {
            writeln!(f, "This Agent {:?} is {:?}", this_agent.0, this_agent.1)?;
        }
        if let Some(this_dna) = &self.this_dna {
            writeln!(f, "This DNA {:?} is {:?}", this_dna.0, this_dna.1)?;
        }
        if let Some(this_agent_info) = &self.this_agent_info {
            writeln!(f, "This agents info: {}", this_agent_info)?;
        }
        for peer in &self.peers {
            writeln!(f, "{}", peer)?;
        }
        Ok(())
    }
}
