use holo_hash::AgentPubKey;
use holo_hash::DhtOpHash;
use holo_hash::DnaHash;
use holochain_state_types::SourceChainDump;
use holochain_types::dht_v2::DhtOp;
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

    /// Ops that are integrated (includes rejected). Integrated **chain ops** are
    /// paged by `dht_ops_cursor`; integrated **warrants** are appended in full on
    /// every call and are not cursor-paged.
    pub integrated: Vec<DhtOp>,

    /// Cursor marking the last integrated **chain op** returned. Pass it to a
    /// subsequent `FullStateDump` to page forward through only the chain ops
    /// integrated since. `None` when no integrated chain ops were returned.
    ///
    /// Only the (unbounded, growing) integrated chain-op list is paged. The two
    /// limbo lists and integrated warrants are bounded/transient and returned in
    /// full on every call, so they can repeat across pages.
    pub dht_ops_cursor: Option<DhtOpsCursor>,
}

/// Pagination cursor for the integrated **chain ops** in a
/// [`FullIntegrationStateDump`]. Warrants and the limbo lists are not paged by it.
///
/// Integrated chain ops are ordered by `(when_integrated, hash)`; a cursor
/// records the last op returned so the next dump resumes strictly after it.
/// The DHT tables are `WITHOUT ROWID`, so pagination is keyed on this
/// timestamp/hash pair rather than a rowid.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DhtOpsCursor {
    /// Microsecond integration timestamp of the last op returned.
    pub when_integrated: i64,
    /// Hash of the last op returned (tie-breaks ops sharing a timestamp).
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
