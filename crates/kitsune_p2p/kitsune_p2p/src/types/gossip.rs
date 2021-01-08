//! # Gossip Event Types

use kitsune_p2p_types::dht_arc::DhtArc;

use crate::agent_store::AgentInfoSigned;

use super::*;

#[derive(Debug, derive_more::Constructor)]
/// Request dht op hashes and
/// agent store information from an agent.
/// This is the lightweight hashes only call.
pub struct ReqOpHashesEvt {
    /// Agent Requesting the ops.
    pub from_agent: Arc<KitsuneAgent>,
    /// The agent you are requesting ops from.
    pub to_agent: Arc<KitsuneAgent>,
    /// The arc on the dht that you want ops from.
    pub dht_arc: DhtArc,
    /// Get ops from this time.
    pub since_utc_epoch_s: i64,
    /// Get ops till this time.
    pub until_utc_epoch_s: i64,
    /// Count of the hashes from the requesting agent
    pub op_count: OpCount,
}

#[derive(Debug, derive_more::Constructor)]
/// Request dht ops from an agent.
pub struct ReqOpDataEvt {
    /// Agent Requesting the ops.
    pub from_agent: Arc<KitsuneAgent>,
    /// The agent you are requesting ops from.
    pub to_agent: Arc<KitsuneAgent>,
    /// The hashes of the ops you want.
    pub op_hashes: Vec<Arc<KitsuneOpHash>>,
    /// The hashes of the agent store information you want.
    pub peer_hashes: Vec<Arc<KitsuneAgent>>,
}

#[derive(Debug, derive_more::Constructor)]
/// Request dht ops from an agent.
pub struct GossipEvt {
    /// Agent sending gossip.
    pub from_agent: Arc<KitsuneAgent>,
    /// The agent receiving gossip.
    pub to_agent: Arc<KitsuneAgent>,
    /// The ops being send in this gossip.
    pub ops: Vec<(Arc<KitsuneOpHash>, Vec<u8>)>,
    /// The agent info in this gossip.
    pub agents: Vec<AgentInfoSigned>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
/// Type for responding to a request for dht op hashes.
/// If the count hasn't changed then the hashes are consistent
/// otherwise there is a change in the data and new hashes are sent.
pub enum OpConsistency {
    /// There is new hashes since last gossip request.
    Variance(OpHashes),
    /// Gossip is consistent since the last call.
    Consistent,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
/// The count of ops last seen from the requester.
/// If the requester has a variance then the
/// responder needs to reply with hashes.
pub enum OpCount {
    /// Requestor has a variance since they last
    /// gossiped to this agent.
    Variance,
    /// Requestor is consistent since last gossip
    /// and this is the count they saw from
    /// this agent.
    Consistent(u64),
}

/// Dht Op hashes that an agent holds
pub type OpHashes = Vec<Arc<KitsuneOpHash>>;

/// Dht op and agent hashes that the agent has information on.
pub type OpHashesAgentHashes = (OpConsistency, Vec<(Arc<KitsuneAgent>, u64)>);

/// Dht op and agent hashes that the agent has information on.
/// Same as [OpHashesAgentHashes] but without consistency information.
pub type LocalOpHashesAgentHashes = (OpHashes, Vec<(Arc<KitsuneAgent>, u64)>);
/// The Dht op data and agent store information
pub type OpDataAgentInfo = (Vec<(Arc<KitsuneOpHash>, Vec<u8>)>, Vec<AgentInfoSigned>);
/// Local and remote neighbors.
pub type ListNeighborAgents = (Vec<Arc<KitsuneAgent>>, Vec<Arc<KitsuneAgent>>);

impl Default for OpCount {
    fn default() -> Self {
        OpCount::Variance
    }
}
