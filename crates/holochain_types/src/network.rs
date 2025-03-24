//! Types for interacting with Holochain's network layer.

use holo_hash::DnaHash;
use serde_derive::{Deserialize, Serialize};

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
}
