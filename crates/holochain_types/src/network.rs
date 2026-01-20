//! Types for interacting with Holochain's network layer.

use holo_hash::DnaHash;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;

/// Request network metrics from Kitsune2.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Kitsune2NetworkMetricsRequest {
    /// Request metrics for a specific DNA.
    ///
    /// If this is blank, then metrics for all DNAs will be returned.
    pub dna_hash: Option<DnaHash>,

    /// Include DHT summary in the response.
    pub include_dht_summary: bool,
}

/// Network metrics from Kitsune2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Kitsune2NetworkMetrics {
    /// A summary of the fetch queue.
    ///
    /// The fetch queue is used to retrieve op data based on op ids that have been discovered
    /// through publish or gossip.
    pub fetch_state_summary: kitsune2_api::FetchStateSummary,
    /// A summary of the gossip state.
    ///
    /// This includes both live gossip rounds and metrics about peers that we've gossiped with.
    /// Optionally, it can include a summary of the DHT state as Kitsune2 sees it.
    pub gossip_state_summary: kitsune2_api::GossipStateSummary,

    /// A summary of the state of each local agent.
    pub local_agents: Vec<LocalAgentSummary>,
}

/// Summary of a local agent's network state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalAgentSummary {
    /// The agent's public key.
    pub agent: holo_hash::AgentPubKey,

    /// The current storage arc that the agent is declaring.
    ///
    /// This is the arc that the agent is claiming that it is an authority for.
    pub storage_arc: kitsune2_api::DhtArc,

    /// The target arc that the agent is trying to achieve as a storage arc.
    ///
    /// This is not declared to other peers on the network. It is used during gossip to try to sync
    /// ops in the target arc. Once the DHT state appears to be in sync with the target arc, the
    /// storage arc can be updated towards the target arc.
    pub target_arc: kitsune2_api::DhtArc,
}

/// Similar struct to [`ApiTransportStats`](kitsune2_api::ApiTransportStats) but with Holochain types.
///
/// There is a [`DnaHash`] instead of a [`Space`](kitsune2_api::Space) in the `blocked_message_counts` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HolochainTransportStats {
    /// Stats for a transport connection.
    pub transport_stats: kitsune2_api::TransportStats,

    /// Blocked message counts.
    pub blocked_message_counts:
        HashMap<kitsune2_api::Url, HashMap<DnaHash, kitsune2_api::MessageBlockCount>>,
}
