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

/// Dht op and agent hashes that the agent has information on.
pub type OpHashesAgentHashes = (Vec<Arc<KitsuneOpHash>>, Vec<(Arc<KitsuneAgent>, u64)>);
/// The Dht op data and agent store information
pub type OpDataAgentInfo = (Vec<(Arc<KitsuneOpHash>, Vec<u8>)>, Vec<AgentInfoSigned>);
/// Local and remote neighbors.
pub type ListNeighborAgents = (Vec<Arc<KitsuneAgent>>, Vec<Arc<KitsuneAgent>>);
