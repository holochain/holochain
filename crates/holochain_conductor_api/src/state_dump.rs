use holo_hash::AgentPubKey;
use holo_hash::DhtOpHash;
use holo_hash::DnaHash;
pub use holochain_state_types::SourceChainCursor;
use holochain_state_types::SourceChainDump;
use holochain_types::op::DhtOp;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
pub struct JsonDump {
    pub peer_dump: P2pAgentsDump,
    pub source_chain_dump: SourceChainDump,
    pub integration_dump: IntegrationStateDump,
}

#[derive(Serialize, Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct FullStateDump {
    pub peer_dump: P2pAgentsDump,
    pub source_chain_dump: SourceChainDump,
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

    /// Ops that are integrated (includes rejected).
    pub integrated: Vec<DhtOp>,

    /// Cursor marking the last DHT op selected across all lifecycle buckets.
    /// Pass it to a subsequent `FullStateDump` to resume strictly after it.
    /// `None` when the page selected no DHT ops.
    pub dht_ops_cursor: Option<DhtOpsCursor>,
}

/// Pagination cursor for all DHT ops in a [`FullIntegrationStateDump`].
///
/// `(when_received, hash)`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DhtOpsCursor {
    /// Microsecond receipt timestamp of the last op selected.
    pub when_received: i64,
    /// Hash of the last op selected (tie-breaks ops sharing a timestamp).
    pub hash: DhtOpHash,
}

/// State dump of all the peer info
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct P2pAgentsDump {
    /// The info of this agents cell.
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
    pub kitsune_agent: Arc<kitsune2_api::AgentId>,
    pub kitsune_space: Arc<kitsune2_api::SpaceId>,
    pub dump: String,
}

impl std::fmt::Display for JsonDump {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let num_other_peers = self.peer_dump.peers.len();
        let s = &self.source_chain_dump;
        writeln!(f, "--- Cell State Dump Summary ---")?;
        writeln!(f, "Number of other peers in p2p store: {num_other_peers},")?;
        writeln!(
            f,
            "Records returned: {}, Ops published: {}",
            s.records.len(),
            s.published_ops_count
        )
    }
}

impl std::fmt::Display for IntegrationStateDumps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        for i in &self.0 {
            write!(f, "{i},")?;
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
            writeln!(f, "This agents info: {this_agent_info}")?;
        }
        for peer in &self.peers {
            writeln!(f, "{peer}")?;
        }
        Ok(())
    }
}
