//! Definitions related to the KitsuneP2p peer-to-peer / dht communications actor.

use std::sync::Arc;

/// Publish data to a "neighborhood" of remote nodes surrounding the "basis" hash.
/// Returns an approximate number of nodes reached.
#[derive(Clone, Debug)]
pub struct Broadcast {
    /// The "space" context.
    pub space: Arc<super::KitsuneSpace>,
    /// The "basis" hash/coordinate of destination neigborhood.
    pub basis: Arc<super::KitsuneBasis>,
    /// The desired count of remote nodes to reach.
    /// Kitsune will keep searching for new nodes to broadcast to until:
    ///  - (A) this target count is reached, or
    ///  - (B) the below timeout is exceeded.
    /// Set to None if you just want a default best-effort.
    pub remote_agent_count: Option<u8>,
    /// The timeout to await for sucessful broadcasts.
    /// Set to None if you don't care to get a count -
    /// broadcast will immediately return 0, but give a best effort to meet
    /// remote_agent_count.
    pub timeout_ms: Option<u64>,
    /// Broadcast data.
    pub broadcast: Vec<u8>,
}

/// Make a request to multiple destination agents - awaiting/aggregating the responses.
/// The remote sides will see these messages as "RequestEvt" events.
#[derive(Clone, Debug)]
pub struct MultiRequest {
    /// The "space" context.
    pub space: Arc<super::KitsuneSpace>,
    /// The agent making the request.
    pub from_agent: Arc<super::KitsuneAgent>,
    /// The "basis" hash/coordinate of destination neigborhood.
    pub basis: Arc<super::KitsuneBasis>,
    /// See docs on Broadcast
    pub remote_agent_count: Option<u8>,
    /// See docs on Broadcast
    pub timeout_ms: Option<u64>,
    /// We are interested in speed. If `true` and we have any results
    /// when `race_timeout_ms` is expired, those results will be returned.
    /// After `race_timeout_ms` and before `timeout_ms` the first result
    /// received will be returned.
    pub as_race: bool,
    /// See `as_race` for details.
    /// Set to `None` for a default "best-effort" race.
    pub race_timeout_ms: Option<u64>,
    /// Request data.
    pub request: Vec<u8>,
}

/// A response type helps indicate what agent gave what response.
#[derive(Clone, Debug)]
pub struct MultiRequestResponse {
    /// The agent that gave this response.
    pub agent: Arc<super::KitsuneAgent>,
    /// Response data.
    pub response: Vec<u8>,
}

ghost_actor::ghost_actor! {
    /// The KitsuneP2pSender allows async remote-control of the KitsuneP2p actor.
    pub actor KitsuneP2p<super::KitsuneP2pError> {
        /// Announce a space/agent pair on this network.
        fn join(space: Arc<super::KitsuneSpace>, agent: Arc<super::KitsuneAgent>) -> ();

        /// Withdraw this space/agent pair from this network.
        fn leave(space: Arc<super::KitsuneSpace>, agent: Arc<super::KitsuneAgent>) -> ();

        /// Make a request of a remote agent.
        fn request(space: Arc<super::KitsuneSpace>, agent: Arc<super::KitsuneAgent>, data: Vec<u8>) -> Vec<u8>;

        /// Publish data to a "neighborhood" of remote nodes surrounding the "basis" hash.
        /// Returns an approximate number of nodes reached.
        fn broadcast(input: Broadcast) -> u8;

        /// Make a request to multiple destination agents - awaiting/aggregating the responses.
        /// The remote sides will see these messages as "RequestEvt" events.
        fn multi_request(input: MultiRequest) -> Vec<MultiRequestResponse>;
    }
}
