//! Types used for dumping the state of a holochain conductor and its cells.

use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_state_types::SourceChainDump;
use holochain_types::dht_op::DhtOp;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;

/// A state dump for a cell.
///
/// For more details, obtain a [`FullStateDump`].
#[derive(Serialize, Deserialize)]
pub struct CellStateDump {
    /// Local agents and peers participating in this cell's network.
    pub peer_dump: P2pAgentsDump,
    /// The source chain dump for this cell.
    pub source_chain_dump: SourceChainDump,
    /// Dump of validation and integration states for ops that are still being processed.
    pub integration_dump: IntegrationStateDump,
}

/// A more detailed state dump for a cell.
///
/// See also [`CellStateDump`].
#[derive(Serialize, Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct FullStateDump {
    /// Local agents and peers participating in this cell's network.
    pub peer_dump: P2pAgentsDump,
    /// The source chain dump for this cell.
    pub source_chain_dump: SourceChainDump,
    /// Detailed dump of validation and integration states for ops that are still being processed.
    pub integration_dump: FullIntegrationStateDump,
}

/// A collection of many cells dumps for easy viewing.
/// Use display to see a nice printout.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IntegrationStateDumps(pub Vec<IntegrationStateDump>);

/// A high level view of the incoming ops and where
/// they are currently.
/// Ops start in the validation limbo then proceed
/// to the integration limbo then finally are integrated.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IntegrationStateDump {
    /// Ops in validation limbo awaiting sys
    /// or app validation.
    pub validation_limbo: usize,
    /// Ops waiting to be integrated.
    pub integration_limbo: usize,
    /// Ops that are integrated.
    /// This includes rejected.
    pub integrated: usize,
}

/// A full view of the DHT shard of the Cell.
/// Ops start in the validation limbo then proceed
/// to the integration limbo then finally are integrated.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct FullIntegrationStateDump {
    /// Ops in validation limbo awaiting sys
    /// or app validation.
    pub validation_limbo: Vec<DhtOp>,

    /// Ops waiting to be integrated.
    pub integration_limbo: Vec<DhtOp>,

    /// Ops that are integrated.
    /// This includes rejected.
    pub integrated: Vec<DhtOp>,

    /// RowId for the latest DhtOp that we have seen
    /// Useful for subsequent calls to `FullStateDump`
    /// to return only what they haven't seen
    pub dht_ops_cursor: u64,
}

/// State dump of all the peer info
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct P2pAgentsDump {
    /// The info of this agent's cell.
    pub this_agent_info: Option<AgentInfoDump>,
    /// The dna as a [`DnaHash`] and [`kitsune2_api::SpaceId`].
    pub this_dna: Option<(DnaHash, kitsune2_api::SpaceId)>,
    /// The agent as [`AgentPubKey`] and [`kitsune2_api::AgentId`].
    pub this_agent: Option<(AgentPubKey, kitsune2_api::AgentId)>,
    /// All other agent info.
    pub peers: Vec<AgentInfoDump>,
}

/// Agent info dump with the agent,
/// space, signed time, expires in and
/// urls printed in a pretty way.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AgentInfoDump {
    /// The network representation of an agent public key.
    pub kitsune_agent: Arc<kitsune2_api::AgentId>,
    /// The network representation of a DNA hash.
    pub kitsune_space: Arc<kitsune2_api::SpaceId>,
    /// The agent info that this agent published to the network.
    pub dump: String,
}

impl std::fmt::Display for CellStateDump {
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
            "Records authored: {}, Ops published: {}",
            s.records.len(),
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
