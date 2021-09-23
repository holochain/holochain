//! Definitions for events emited from the KitsuneP2p actor.

use crate::types::agent_store::AgentInfoSigned;
use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::dht_arc::{DhtArcSet, DhtLocation};
use std::{collections::HashSet, sync::Arc, time::SystemTime};

/// Gather a list of op-hashes from our implementor that meet criteria.
/// Also get the start and end times for ops within a time window
/// up to a maximum number.
#[derive(Debug)]
pub struct QueryOpHashesEvt {
    /// The "space" context.
    pub space: KSpace,
    /// The agents from which to fetch, along with a DhtArcSet to filter by.
    pub agents: Vec<(KAgent, DhtArcSet)>,
    /// The time window to search within.
    pub window: TimeWindow,
    /// Maximum number of ops to return.
    pub max_ops: usize,
    /// Include ops that are still in limbo (not yet validated or integrated).
    pub include_limbo: bool,
}

/// Gather all op-hash data for a list of op-hashes from our implementor.
#[derive(Debug)]
pub struct FetchOpDataEvt {
    /// The "space" context.
    pub space: KSpace,
    /// The "agent" context.
    pub agents: Vec<KAgent>,
    /// The op-hashes to fetch
    pub op_hashes: Vec<KOpHash>,
}

/// Request that our implementor sign some data on behalf of an agent.
#[derive(Debug)]
pub struct SignNetworkDataEvt {
    /// The "space" context.
    pub space: KSpace,
    /// The "agent" context.
    pub agent: KAgent,
    /// The data to sign.
    #[allow(clippy::rc_buffer)]
    pub data: Arc<Vec<u8>>,
}

/// Store the AgentInfo as signed by the agent themselves.
#[derive(Debug)]
pub struct PutAgentInfoSignedEvt {
    /// The "space" context.
    pub space: KSpace,
    /// A batch of signed agent info for this space.
    pub peer_data: Vec<AgentInfoSigned>,
}

/// Get agent info for a single agent, as previously signed and put.
#[derive(Debug)]
pub struct GetAgentInfoSignedEvt {
    /// The "space" context.
    pub space: KSpace,
    /// The "agent" context.
    pub agent: KAgent,
}

/// Get agent info which satisfies a query.
#[derive(Debug)]
pub struct QueryAgentsEvt {
    /// The "space" context.
    pub space: KSpace,
    /// Optional set of agents to filter by.
    pub agents: Option<HashSet<KAgent>>,
    /// Optional time range to filter by.
    pub window: Option<TimeWindow>,
    /// Optional arcset to intersect by.
    pub arc_set: Option<Arc<DhtArcSet>>,
    /// If set, results are ordered by proximity to the specified location
    pub near_basis: Option<DhtLocation>,
    /// Limit to the number of results returned
    pub limit: Option<u32>,
}

// NB: if we want to play it safer, rather than providing these fine-grained
//     builder methods, we could provide only the three "flavors" of query that
//     Holochain supports, which would still provide us the full expressivity to
//     implement Kitsune.
impl QueryAgentsEvt {
    /// Constructor. Every query needs to know what space it's for.
    pub fn new(space: KSpace) -> Self {
        Self {
            space,
            agents: None,
            window: None,
            arc_set: None,
            near_basis: None,
            limit: None,
        }
    }

    /// Add in an agent list query
    pub fn by_agents<A: IntoIterator<Item = KAgent>>(mut self, agents: A) -> Self {
        self.agents = Some(agents.into_iter().collect());
        self
    }

    /// Add in an a time window query
    pub fn by_window(mut self, window: TimeWindow) -> Self {
        self.window = Some(window);
        self
    }

    /// Add in an an arcset query
    pub fn by_arc_set(mut self, arc_set: Arc<DhtArcSet>) -> Self {
        self.arc_set = Some(arc_set);
        self
    }

    /// Specify that the results should be ordered by proximity to this basis
    // TODO: make it take a DhtLocation
    pub fn near_basis(mut self, basis: u32) -> Self {
        self.near_basis = Some(DhtLocation::new(basis));
        self
    }

    /// Limit the number of results
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }
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
    pub agent: KAgent,
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
        agent: KAgent,
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
    Oldest(Option<KAgent>),
}

/// A range of timestamps, measured in milliseconds
pub type TimeWindow = std::ops::Range<Timestamp>;

/// A time window which covers all of recordable time
pub fn full_time_range() -> TimeWindow {
    Timestamp::MIN..Timestamp::MAX
}
type KSpace = Arc<super::KitsuneSpace>;
type KAgent = Arc<super::KitsuneAgent>;
type KOpHash = Arc<super::KitsuneOpHash>;
type Payload = Vec<u8>;
type Ops = Vec<(KOpHash, Payload)>;

ghost_actor::ghost_chan! {
    /// The KitsuneP2pEvent stream allows handling events generated from the
    /// KitsuneP2p actor.
    pub chan KitsuneP2pEvent<super::KitsuneP2pError> {
        /// We need to store signed agent info.
        fn put_agent_info_signed(input: PutAgentInfoSignedEvt) -> ();

        /// We need to get previously stored agent info.
        fn get_agent_info_signed(input: GetAgentInfoSignedEvt) -> Option<crate::types::agent_store::AgentInfoSigned>;

        /// We need to get previously stored agent info.
        fn query_agents(input: QueryAgentsEvt) -> Vec<crate::types::agent_store::AgentInfoSigned>;

        /// Query the peer density of a space for a given [`DhtArc`].
        fn query_peer_density(space: KSpace, dht_arc: kitsune_p2p_types::dht_arc::DhtArc) -> kitsune_p2p_types::dht_arc::PeerDensity;

        /// Record a metric datum about an agent.
        fn put_metric_datum(datum: MetricDatum) -> ();

        /// Ask for metric data.
        fn query_metrics(query: MetricQuery) -> MetricQueryAnswer;

        /// We are receiving a request from a remote node.
        fn call(space: KSpace, to_agent: KAgent, from_agent: KAgent, payload: Payload) -> Vec<u8>;

        /// We are receiving a notification from a remote node.
        fn notify(space: KSpace, to_agent: KAgent, from_agent: KAgent, payload: Payload) -> ();

        /// We are receiving a dht op we may need to hold distributed via gossip.
        fn gossip(space: KSpace, to_agent: KAgent, ops: Ops) -> ();

        /// Gather a list of op-hashes from our implementor that meet criteria.
        /// Get the oldest and newest times for ops within a time window and max number of ops.
        // maackle: do we really need to *individually* wrap all these op hashes in Arcs?
        fn query_op_hashes(input: QueryOpHashesEvt) -> Option<(Vec<KOpHash>, TimeWindow)>;

        /// Gather all op-hash data for a list of op-hashes from our implementor.
        fn fetch_op_data(input: FetchOpDataEvt) -> Vec<(KOpHash, Vec<u8>)>;

        /// Request that our implementor sign some data on behalf of an agent.
        fn sign_network_data(input: SignNetworkDataEvt) -> super::KitsuneSignature;
    }
}

/// Receiver type for incoming connection events.
pub type KitsuneP2pEventReceiver = futures::channel::mpsc::Receiver<KitsuneP2pEvent>;
