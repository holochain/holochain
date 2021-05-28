//! Definitions for events emited from the KitsuneP2p actor.

use crate::types::agent_store::AgentInfoSigned;
use std::sync::Arc;

/// Gather a list of op-hashes from our implementor that meet criteria.
#[derive(Debug)]
pub struct FetchOpHashesForConstraintsEvt {
    /// The "space" context.
    pub space: Arc<super::KitsuneSpace>,
    /// The "agent" context.
    pub agent: Arc<super::KitsuneAgent>,
    /// The dht arc to query.
    pub dht_arc: kitsune_p2p_types::dht_arc::DhtArc,
    /// Only retreive items received since this time (INCLUSIVE).
    pub since_utc_epoch_s: i64,
    /// Only retreive items received until this time (EXCLUSIVE).
    pub until_utc_epoch_s: i64,
}

/// Gather all op-hash data for a list of op-hashes from our implementor.
#[derive(Debug)]
pub struct FetchOpHashDataEvt {
    /// The "space" context.
    pub space: Arc<super::KitsuneSpace>,
    /// The "agent" context.
    pub agent: Arc<super::KitsuneAgent>,
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

#[derive(Debug)]
/// Store the AgentInfo as signed by the agent themselves.
pub struct PutAgentInfoSignedEvt {
    /// The "space" context.
    pub space: Arc<super::KitsuneSpace>,
    /// The "agent" context.
    pub agent: Arc<super::KitsuneAgent>,
    /// The signed agent info.
    pub agent_info_signed: AgentInfoSigned,
}

#[derive(Debug)]
/// Get agent info as previously signed and put.
pub struct GetAgentInfoSignedEvt {
    /// The "space" context.
    pub space: Arc<super::KitsuneSpace>,
    /// The "agent" context.
    pub agent: Arc<super::KitsuneAgent>,
}

#[derive(Debug)]
/// Get agent info as previously signed and put.
pub struct QueryAgentInfoSignedEvt {
    /// The "space" context.
    pub space: Arc<super::KitsuneSpace>,
    /// The "agent" context.
    pub agent: Arc<super::KitsuneAgent>,
}

/// A single datum of metric info about an Agent, to be recorded by the client.
#[derive(derive_more::Display)]
pub enum MetricDatumKind {
    /// Our fast gossip loop synced this node up to this timestamp.
    /// The next quick loop can sync from this timestamp forward.
    LastQuickGossip,

    /// The last time a full slow gossip loop completed was at this timestamp.
    /// If that is too recent, we won't run another slow loop.
    LastSlowGossip,

    /// The last time we got a connection/timeout error with this node,
    /// ignoring inactivity timeouts.
    /// Lets us skip recently unreachable nodes in gossip loops.
    LastConnectError,
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
        last_connect_error_threshold: std::time::Instant,
    },
}

/// Corresponding response to `MetricQuery`
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetricQueryAnswer {
    /// The last sync time for all agents.
    LastSync(std::time::Instant),
    /// The agent with the oldest last-connection time which satisfies the query.
    Oldest(super::KitsuneAgent),
}

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

        /// Record a metric datum about an agent.
        fn put_metric_datum(agent: Arc<super::KitsuneAgent>, metric: MetricDatumKind) -> ();

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
        fn fetch_op_hashes_for_constraints(input: FetchOpHashesForConstraintsEvt) -> Vec<Arc<super::KitsuneOpHash>>;

        /// Gather all op-hash data for a list of op-hashes from our implementor.
        fn fetch_op_hash_data(input: FetchOpHashDataEvt) -> Vec<(Arc<super::KitsuneOpHash>, Vec<u8>)>;

        /// Request that our implementor sign some data on behalf of an agent.
        fn sign_network_data(input: SignNetworkDataEvt) -> super::KitsuneSignature;
    }
}

/// Receiver type for incoming connection events.
pub type KitsuneP2pEventReceiver = futures::channel::mpsc::Receiver<KitsuneP2pEvent>;
