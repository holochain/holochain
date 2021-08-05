//! Definitions related to the KitsuneP2p peer-to-peer / dht communications actor.

use kitsune_p2p_types::config::KitsuneP2pTuningParams;
use kitsune_p2p_types::KitsuneTimeout;
use std::sync::Arc;
use url2::Url2;

/// Make a request to multiple destination agents - awaiting/aggregating the responses.
/// The remote sides will see these messages as "RequestEvt" events.
#[derive(Clone, Debug)]
pub struct RpcMulti {
    /// The "space" context.
    pub space: Arc<super::KitsuneSpace>,

    /// The agent making the request.
    pub from_agent: Arc<super::KitsuneAgent>,

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
        from_agent: Arc<super::KitsuneAgent>,
        basis: Arc<super::KitsuneBasis>,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            space,
            from_agent,
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

type KSpace = Arc<super::KitsuneSpace>;
type KAgent = Arc<super::KitsuneAgent>;
type KBasis = Arc<super::KitsuneBasis>;
type Payload = Vec<u8>;
type OptU64 = Option<u64>;

ghost_actor::ghost_chan! {
    /// The KitsuneP2pSender allows async remote-control of the KitsuneP2p actor.
    pub chan KitsuneP2p<super::KitsuneP2pError> {
        /// Get the calculated transport bindings.
        fn list_transport_bindings() -> Vec<Url2>;

        /// Announce a space/agent pair on this network.
        fn join(space: KSpace, agent: KAgent) -> ();

        /// Withdraw this space/agent pair from this network.
        fn leave(space: KSpace, agent: KAgent) -> ();

        /// Make a request of a single remote agent, expecting a response.
        /// The remote side will receive a "Call" event.
        fn rpc_single(space: KSpace, to_agent: KAgent, from_agent: KAgent, payload: Payload, timeout_ms: OptU64) -> Vec<u8>;

        /// Make a request to multiple destination agents - awaiting/aggregating the responses.
        /// The remote sides will see these messages as "Call" events.
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
            payload: Payload
        ) -> ();

        /// New integrated data.
        fn new_integrated_data(space: KSpace) -> ();

        /// Check if an agent is an authority for a hash.
        fn authority_for_hash(
            space: KSpace,
            agent: KAgent,
            basis: KBasis,
        ) -> bool;
    }
}
