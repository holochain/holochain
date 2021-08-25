//! Types used for consistency checking during tests or dht health checks.
//! These types describe a protocol that can be implemented to gather statistics
//! on data consistency.
//! This is a first prototype so expect this to change.
//!
//! The idea is that a central node can request all the published hashes from a set
//! of nodes on the DHT and then create a consistency session for each node.
//! The sessions can then be sent to each node so they can self-check what they
//! should be holding and then send back small session reports at a set frequency.
//!
//! This allows consistency and health checks to be run at scale with minimal network traffic.
//! It does require honest network nodes. (Although this could be strengthened).
use std::time::Duration;

use crate::bin_types::*;
use dht_arc::DhtArc;

use super::*;

/// Data published by an agent.
pub struct PublishedData {
    /// The agent that published the data.
    pub agent: Arc<KitsuneAgent>,
    /// The storage arc of the agent.
    pub storage_arc: DhtArc,
    /// The op hashes published by the agent.
    pub published_hashes: Vec<KitsuneOpHash>,
}

/// A consistency session for an individual agent
/// to self check and report back.
pub struct ConsistencySession {
    /// How often the agent should send keep alives if they
    /// do not have all the expected data yet.
    pub keep_alive: Option<Duration>,
    /// How often the agent should check if they have all the
    /// expected data.
    pub frequency: Duration,
    /// When the agent should timeout the session.
    pub timeout: Duration,
    /// The data the agent should check for.
    pub expected_data: ExpectedData,
}

/// The data the agent is expected to have.
pub struct ExpectedData {
    /// The agents this agent is expected to have in their peer store.
    pub expected_agents: Vec<Arc<KitsuneAgent>>,
    /// The ops this agent is expected to have integrated.
    pub expected_hashes: Vec<KitsuneOpHash>,
}

/// A message from an agent with a report on their status for this session.
pub struct SessionMessage {
    /// The agent that sent the message.
    pub from: Arc<KitsuneAgent>,
    /// The status report.
    pub report: SessionReport,
}

/// The status of this agents session.
pub enum SessionReport {
    /// The session is still running and the agent is missing data.
    KeepAlive {
        /// The number of missing agents.
        missing_agents: u32,
        /// The expected number of hashes.
        out_of_agents: u32,
        /// The number of missing ops.
        missing_hashes: u32,
        /// The expected number of hashes.
        out_of_hashes: u32,
    },
    /// The session is complete and the agent has all the data.
    Complete {
        /// The time it took to complete the session.
        elapsed_ms: u32,
    },
    /// The session timed out and the agent is still missing data.
    Timeout {
        /// The agents that are missing.
        missing_agents: Vec<Arc<KitsuneAgent>>,
        /// The ops that ars missing.
        missing_hashes: Vec<KitsuneOpHash>,
    },
    /// An error has occurred and the session has failed for this agent.
    Error {
        /// The error that occurred.
        error: String,
    },
}
