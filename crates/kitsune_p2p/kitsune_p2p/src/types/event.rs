//! Definitions for events emited from the KitsuneP2p actor.

use crate::types::agent_store::AgentInfoSigned;
use kitsune_p2p_types::dht_arc::DhtArcSet;
use std::{sync::Arc, time::SystemTime};

/// Gather a list of op-hashes from our implementor that meet criteria.
/// Also get the start and end times for ops within a time window
/// up to a maximum number.
#[derive(Debug)]
pub struct FetchOpHashesForConstraintsEvt {
    /// The "space" context.
    pub space: Arc<super::KitsuneSpace>,
    /// The agents from which to fetch, along with a DhtArcSet to filter by.
    pub agents: Vec<(Arc<super::KitsuneAgent>, DhtArcSet)>,
    /// The time window to search within.
    pub window_ms: TimeWindowMs,
    /// Maximum number of ops to return.
    pub max_ops: usize,
}

/// Gather all op-hash data for a list of op-hashes from our implementor.
#[derive(Debug)]
pub struct FetchOpHashDataEvt {
    /// The "space" context.
    pub space: Arc<super::KitsuneSpace>,
    /// The "agent" context.
    pub agents: Vec<Arc<super::KitsuneAgent>>,
    /// The op-hashes to fetch
    pub op_hashes: Vec<Arc<super::KitsuneOpHash>>,
}

/// Request that our implementor sign some data on behalf of an agent.
#[derive(Debug)]
pub struct SignNetworkDataEvt {
    /// The "space" context.
    pub space: Arc<super::KitsuneSpace>,
    /// The "agent" context.
    pub agent: Arc<super::KitsuneAgent>,
    /// The data to sign.
    #[allow(clippy::rc_buffer)]
    pub data: Arc<Vec<u8>>,
}

/// Store the AgentInfo as signed by the agent themselves.
#[derive(Debug)]
pub struct PutAgentInfoSignedEvt {
    /// The "space" context.
    pub space: Arc<super::KitsuneSpace>,
    /// The "agent" context.
    pub agent: Arc<super::KitsuneAgent>,
    /// The signed agent info.
    pub agent_info_signed: AgentInfoSigned,
}

/// Get agent info for a single agent, as previously signed and put.
#[derive(Debug)]
pub struct GetAgentInfoSignedEvt {
    /// The "space" context.
    pub space: Arc<super::KitsuneSpace>,
    /// The "agent" context.
    pub agent: Arc<super::KitsuneAgent>,
}

/// Get agent info which satisfies a query.
#[derive(Debug)]
pub struct QueryAgentInfoSignedEvt {
    /// The "space" context.
    pub space: Arc<super::KitsuneSpace>,
    /// The optional list of agents to filter by.
    pub agents: Option<Vec<Arc<super::KitsuneAgent>>>,
}

/// Get agent info which satisfies a query.
#[derive(Debug)]
pub struct QueryGossipAgentsEvt {
    /// The "space" context.
    pub space: Arc<super::KitsuneSpace>,
    /// The optional list of agents to filter by.
    pub agents: Option<Vec<Arc<super::KitsuneAgent>>>,
    /// Start of the time window.
    pub since_ms: u64,
    /// End of the time window.
    pub until_ms: u64,
    /// The set that we need agents to fit into.
    pub arc_set: Arc<kitsune_p2p_types::dht_arc::DhtArcSet>,
}

/// A single datum of metric info about an Agent, to be recorded by the client.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, derive_more::Display)]
pub enum MetricKind {
    /// Our fast gossip loop synced this node up to this timestamp.
    /// The next quick loop can sync from this timestamp forward.
    QuickGossip,

    /// The last time a full slow gossip loop completed was at this timestamp.
    /// If that is too recent, we won't run another slow loop.
    SlowGossip,

    /// The last time we got a connection/timeout error with this node,
    /// ignoring inactivity timeouts.
    /// Lets us skip recently unreachable nodes in gossip loops.
    ConnectError,
}

/// A single row in the metrics database
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetricDatum {
    /// The agent this event is about
    pub agent: Arc<super::KitsuneAgent>,
    /// The kind of event
    pub kind: MetricKind,
    /// The time at which this occurred
    pub timestamp: SystemTime,
}

/// The ordering is defined as such to facilitate in-memory metric store
/// implementations such that the earliest and latest metrics can be easily obtained.
impl PartialOrd for MetricDatum {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.timestamp.cmp(&other.timestamp) {
            std::cmp::Ordering::Equal => Some(self.agent.cmp(&other.agent)),
            o => Some(o),
        }
    }
}

impl Ord for MetricDatum {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

/// Different kinds of queries about metric data
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetricQuery {
    /// Filters for the "last sync" query.
    LastSync {
        /// The agent to query by
        agent: Arc<super::KitsuneAgent>,
    },
    /// Filters for the "oldest agent" query.
    Oldest {
        /// Agents whose last connection error is earlier than this time will be filtered out.
        last_connect_error_threshold: std::time::SystemTime,
    },
}

/// Corresponding response to `MetricQuery`
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetricQueryAnswer {
    /// The last sync time for all agents.
    LastSync(Option<std::time::SystemTime>),
    /// The agent with the oldest last-connection time which satisfies the query.
    Oldest(Option<Arc<super::KitsuneAgent>>),
}

/// A UNIX timestamp, measured in milliseconds
pub type TimestampMs = u64;

/// A range of timestamps, measured in milliseconds
pub type TimeWindowMs = std::ops::Range<TimestampMs>;

ghost_actor::ghost_chan! {
    /// The KitsuneP2pEvent stream allows handling events generated from the
    /// KitsuneP2p actor.
    pub chan KitsuneP2pEvent<super::KitsuneP2pError> {
        /// We need to store signed agent info.
        fn put_agent_info_signed(input: PutAgentInfoSignedEvt) -> ();

        /// We need to get previously stored agent info.
        fn get_agent_info_signed(input: GetAgentInfoSignedEvt) -> Option<crate::types::agent_store::AgentInfoSigned>;

        /// We need to get previously stored agent info.
        fn query_agent_info_signed(input: QueryAgentInfoSignedEvt) -> Vec<crate::types::agent_store::AgentInfoSigned>;

        /// We need to get agents that fit into an arc set for gossip.
        fn query_gossip_agents(input: QueryGossipAgentsEvt) -> Vec<(Arc<crate::KitsuneAgent>, kitsune_p2p_types::dht_arc::ArcInterval)>;

        /// query agent info in order of closeness to a basis location.
        fn query_agent_info_signed_near_basis(space: Arc<super::KitsuneSpace>, basis_loc: u32, limit: u32) -> Vec<crate::types::agent_store::AgentInfoSigned>;

        /// Record a metric datum about an agent.
        fn put_metric_datum(datum: MetricDatum) -> ();

        /// Ask for metric data.
        fn query_metrics(query: MetricQuery) -> MetricQueryAnswer;

        /// We are receiving a request from a remote node.
        fn call(space: Arc<super::KitsuneSpace>, to_agent: Arc<super::KitsuneAgent>, from_agent: Arc<super::KitsuneAgent>, payload: Vec<u8>) -> Vec<u8>;

        /// We are receiving a notification from a remote node.
        fn notify(space: Arc<super::KitsuneSpace>, to_agent: Arc<super::KitsuneAgent>, from_agent: Arc<super::KitsuneAgent>, payload: Vec<u8>) -> ();

        /// We are receiving a dht op we may need to hold distributed via gossip.
        fn gossip(
            space: Arc<super::KitsuneSpace>,
            to_agent: Arc<super::KitsuneAgent>,
            from_agent: Arc<super::KitsuneAgent>,
            op_hash: Arc<super::KitsuneOpHash>,
            op_data: Vec<u8>,
        ) -> ();

        /// Gather a list of op-hashes from our implementor that meet criteria.
        /// Get the oldest and newest times for ops within a time window and max number of ops.
        fn fetch_op_hashes_for_constraints(input: FetchOpHashesForConstraintsEvt) -> Option<(Vec<Arc<super::KitsuneOpHash>>, TimeWindowMs)>;

        /// Gather all op-hash data for a list of op-hashes from our implementor.
        fn fetch_op_hash_data(input: FetchOpHashDataEvt) -> Vec<(Arc<super::KitsuneOpHash>, Vec<u8>)>;

        /// Request that our implementor sign some data on behalf of an agent.
        fn sign_network_data(input: SignNetworkDataEvt) -> super::KitsuneSignature;
    }
}

/// Receiver type for incoming connection events.
pub type KitsuneP2pEventReceiver = futures::channel::mpsc::Receiver<KitsuneP2pEvent>;
