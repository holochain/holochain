//! Definitions related to the KitsuneP2p peer-to-peer / dht communications actor.

use kitsune_p2p_types::config::KitsuneP2pTuningParams;
use kitsune_p2p_types::KitsuneTimeout;
use std::sync::Arc;
use url2::Url2;

use crate::gossip::sharded_gossip::KitsuneDiagnostics;

/// Make a request to multiple destination agents - awaiting/aggregating the responses.
/// The remote sides will see these messages as "RequestEvt" events.
#[derive(Clone, Debug)]
pub struct RpcMulti {
    /// The "space" context.
    pub space: Arc<super::KitsuneSpace>,

    /// The "basis" hash/coordinate of destination neigborhood.
    pub basis: Arc<super::KitsuneBasis>,

    /// Request data.
    pub payload: Vec<u8>,

    /// Max number of remote requests to make
    pub max_remote_agent_count: u8,

    /// Max timeout for aggregating response data
    pub max_timeout: KitsuneTimeout,

    /// Remote request grace period.
    /// If we already have results from other sources,
    /// but made any additional outgoing remote requests,
    /// we'll wait at least this long for additional responses.
    pub remote_request_grace_ms: u64,
}

impl RpcMulti {
    /// Construct a new RpcMulti input struct
    /// with timing defaults specified by tuning_params.
    pub fn new(
        tuning_params: &KitsuneP2pTuningParams,
        space: Arc<super::KitsuneSpace>,
        basis: Arc<super::KitsuneBasis>,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            space,
            basis,
            payload,
            max_remote_agent_count: tuning_params.default_rpc_multi_remote_agent_count,
            max_timeout: tuning_params.implicit_timeout(),
            remote_request_grace_ms: tuning_params.default_rpc_multi_remote_request_grace_ms,
        }
    }
}

/// A response type helps indicate what agent gave what response.
#[derive(Clone, Debug)]
pub struct RpcMultiResponse {
    /// The agent that gave this response.
    pub agent: Arc<super::KitsuneAgent>,
    /// Response data.
    pub response: Vec<u8>,
}

/// Data to broadcast to the remote.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BroadcastData {
    /// User broadcast.
    User(#[serde(with = "serde_bytes")] Vec<u8>),

    /// Agent info.
    AgentInfo(kitsune_p2p_types::agent_info::AgentInfoSigned),

    /// Publish broadcast.
    Publish(
        Vec<kitsune_p2p_fetch::OpHashSized>,
        kitsune_p2p_fetch::FetchContext,
    ),
}

type KSpace = Arc<super::KitsuneSpace>;
type KSpaceOpt = Option<Arc<super::KitsuneSpace>>;
type KAgent = Arc<super::KitsuneAgent>;
type KAgents = Vec<Arc<super::KitsuneAgent>>;
type KBasis = Arc<super::KitsuneBasis>;
type Payload = Vec<u8>;
type OptU64 = Option<u64>;
type OptArc = Option<crate::dht_arc::DhtArc>;

ghost_actor::ghost_chan! {
    /// The KitsuneP2pSender allows async remote-control of the KitsuneP2p actor.
    pub chan KitsuneP2p<super::KitsuneP2pError> {
        /// Get the calculated transport bindings.
        fn list_transport_bindings() -> Vec<Url2>;

        /// Announce a space/agent pair on this network.
        fn join(space: KSpace, agent: KAgent, initial_arc: OptArc) -> ();

        /// Withdraw this space/agent pair from this network.
        fn leave(space: KSpace, agent: KAgent) -> ();

        /// Make a request of a single remote agent, expecting a response.
        /// The remote side will receive a "Call" event.
        fn rpc_single(space: KSpace, to_agent: KAgent, payload: Payload, timeout_ms: OptU64) -> Vec<u8>;

        /// Make a request to multiple destination agents - awaiting/aggregating the responses.
        /// The remote sides will see these messages as "Call" events.
        /// NOTE: We've currently disabled the "multi" part of this.
        /// It will still pick appropriate peers by basis, but will only
        /// make requests one at a time, returning the first success.
        fn rpc_multi(input: RpcMulti) -> Vec<RpcMultiResponse>;

        /// Publish data to a "neighborhood" of remote nodes surrounding the
        /// "basis" hash. This is a multi-step fire-and-forget algorithm.
        /// An Ok(()) result only means that we were able to establish at
        /// least one connection with a node in the target neighborhood.
        /// The remote sides will see these messages as "Notify" events.
        fn broadcast(
            space: KSpace,
            basis: KBasis,
            timeout: KitsuneTimeout,
            data: BroadcastData,
        ) -> ();

        /// Broadcast data to a specific set of agents without
        /// expecting a response.
        /// An Ok(()) result only means that we were able to establish at
        /// least one connection with a node in the agent set.
        fn targeted_broadcast(
            space: KSpace,
            agents: KAgents,
            timeout: KitsuneTimeout,
            payload: Payload,

            // If we have reached the maximum concurrent notify requests limit
            // (specified by tuning param `concurrent_limit_per_thread`)
            // This message will be dropped / not sent, but still return an
            // Ok() response.
            drop_at_limit: bool,
        ) -> ();

        /// New data has been integrated and is ready for gossiping.
        fn new_integrated_data(space: KSpace) -> ();

        /// Check if an agent is an authority for a hash.
        fn authority_for_hash(
            space: KSpace,
            basis: KBasis,
        ) -> bool;

        /// dump network metrics
        fn dump_network_metrics(
            space: KSpaceOpt,
        ) -> serde_json::Value;

        /// Get data for diagnostics
        fn get_diagnostics(space: KSpace) -> KitsuneDiagnostics;
    }
}
