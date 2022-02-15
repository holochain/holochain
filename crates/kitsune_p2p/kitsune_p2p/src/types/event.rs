//! Definitions for events emited from the KitsuneP2p actor.

use crate::types::agent_store::AgentInfoSigned;
use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::{
    bin_types::{KOp, KitsuneOpData},
    dht_arc::{DhtArcSet, DhtLocation},
};
use std::{collections::HashSet, sync::Arc};

/// Gather a list of op-hashes from our implementor that meet criteria.
/// Also get the start and end times for ops within a time window
/// up to a maximum number.
#[derive(Debug)]
pub struct QueryOpHashesEvt {
    /// The "space" context.
    pub space: KSpace,
    /// The DhtArcSet to filter by.
    pub arc_set: DhtArcSet,
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
    pub fn near_basis(mut self, basis: DhtLocation) -> Self {
        self.near_basis = Some(basis);
        self
    }

    /// Limit the number of results
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// An exclusive range of timestamps, measured in microseconds
pub type TimeWindow = std::ops::Range<Timestamp>;

/// An inclusive range of timestamps, measured in microseconds
pub type TimeWindowInclusive = std::ops::RangeInclusive<Timestamp>;

/// A time window which covers all of recordable time
pub fn full_time_window() -> TimeWindow {
    Timestamp::MIN..Timestamp::MAX
}

/// A time window which inclusively covers all of recordable time
pub fn full_time_window_inclusive() -> TimeWindowInclusive {
    Timestamp::MIN..=Timestamp::MAX
}

const METRIC_KIND_UNKNOWN: &str = "Unknown";
const METRIC_KIND_REACHABILITY_QUOTIENT: &str = "ReachabilityQuotient";
const METRIC_KIND_LATENCY_MICROS: &str = "LatencyMicros";
const METRIC_KIND_AGG_EXTRAP_COV: &str = "AggExtrapCov";

/// The type of metric recorded
pub enum MetricRecordKind {
    /// Failure to parse metric kind
    Unknown,

    /// ReachabilityQuotient metric kind
    ReachabilityQuotient,

    /// LatencyMicros metric kind
    LatencyMicros,

    /// AggExtrapCov metric kind
    AggExtrapCov,
}

impl MetricRecordKind {
    /// database format of this kind variant
    pub fn to_db(&self) -> &'static str {
        use MetricRecordKind::*;
        match self {
            Unknown => METRIC_KIND_UNKNOWN,
            ReachabilityQuotient => METRIC_KIND_REACHABILITY_QUOTIENT,
            LatencyMicros => METRIC_KIND_LATENCY_MICROS,
            AggExtrapCov => METRIC_KIND_AGG_EXTRAP_COV,
        }
    }

    /// parse a database kind into a rust enum variant
    pub fn from_db(input: &str) -> Self {
        use MetricRecordKind::*;
        if input == METRIC_KIND_REACHABILITY_QUOTIENT {
            ReachabilityQuotient
        } else if input == METRIC_KIND_LATENCY_MICROS {
            LatencyMicros
        } else if input == METRIC_KIND_AGG_EXTRAP_COV {
            AggExtrapCov
        } else {
            Unknown
        }
    }
}

/// An individual metric record
pub struct MetricRecord {
    /// kind of this record
    pub kind: MetricRecordKind,

    /// agent associated with this metric (if applicable)
    pub agent: Option<Arc<super::KitsuneAgent>>,

    /// timestamp this metric was recorded at
    pub recorded_at_utc: Timestamp,

    /// timestamp this metric will expire and be available for pruning
    pub expires_at_utc: Timestamp,

    /// additional data associated with this metric
    pub data: serde_json::Value,
}

/// Generic Kitsune Request of the implementor
/// This enum may be easier to add variants to for future updates,
/// rather than adding a full new top-level event message type.
pub enum KGenReq {
    /// Extrapolated Peer Coverage
    PeerExtrapCov {
        /// The space to extrapolate coverage
        space: Arc<super::KitsuneSpace>,

        /// Storage arcs of joined agents
        dht_arc_set: DhtArcSet,
    },

    /// Record a set of metric records
    RecordMetrics {
        /// The space to associate the records with
        space: Arc<super::KitsuneSpace>,

        /// The records to record
        records: Vec<MetricRecord>,
    },
}

/// Generic Kitsune Respons from the imlementor
pub enum KGenRes {
    /// Extrapolated Peer Coverage
    PeerExtrapCov(Vec<f64>),
    /// Record a set of metric records
    RecordMetrics(()),
}

type KSpace = Arc<super::KitsuneSpace>;
type KAgent = Arc<super::KitsuneAgent>;
type KOpHash = Arc<super::KitsuneOpHash>;
type Payload = Vec<u8>;
type Ops = Vec<KOp>;

ghost_actor::ghost_chan! {
    /// The KitsuneP2pEvent stream allows handling events generated from the
    /// KitsuneP2p actor.
    pub chan KitsuneP2pEvent<super::KitsuneP2pError> {
        /// Generic Kitsune Request of the implementor
        fn k_gen_req(arg: KGenReq) -> KGenRes;

        /// We need to store signed agent info.
        fn put_agent_info_signed(input: PutAgentInfoSignedEvt) -> ();

        /// We need to get previously stored agent info.
        fn get_agent_info_signed(input: GetAgentInfoSignedEvt) -> Option<crate::types::agent_store::AgentInfoSigned>;

        /// We need to get previously stored agent info.
        fn query_agents(input: QueryAgentsEvt) -> Vec<crate::types::agent_store::AgentInfoSigned>;

        /// Query the peer density of a space for a given [`DhtArc`].
        fn query_peer_density(space: KSpace, dht_arc: kitsune_p2p_types::dht_arc::DhtArc) -> kitsune_p2p_types::dht_arc::PeerViewBeta;

        /// We are receiving a request from a remote node.
        fn call(space: KSpace, to_agent: KAgent, payload: Payload) -> Vec<u8>;

        /// We are receiving a notification from a remote node.
        fn notify(space: KSpace, to_agent: KAgent, payload: Payload) -> ();

        /// We are receiving a dht op we may need to hold distributed via gossip.
        fn gossip(space: KSpace, ops: Ops) -> ();

        /// Gather a list of op-hashes from our implementor that meet criteria.
        /// Get the oldest and newest times for ops within a time window and max number of ops.
        // maackle: do we really need to *individually* wrap all these op hashes in Arcs?
        fn query_op_hashes(input: QueryOpHashesEvt) -> Option<(Vec<KOpHash>, TimeWindowInclusive)>;

        /// Gather all op-hash data for a list of op-hashes from our implementor.
        fn fetch_op_data(input: FetchOpDataEvt) -> Vec<(KOpHash, KOp)>;

        /// Request that our implementor sign some data on behalf of an agent.
        fn sign_network_data(input: SignNetworkDataEvt) -> super::KitsuneSignature;
    }
}

/// Receiver type for incoming connection events.
pub type KitsuneP2pEventReceiver = futures::channel::mpsc::Receiver<KitsuneP2pEvent>;
