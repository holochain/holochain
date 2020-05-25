//! Definitions related to the KitsuneP2p peer-to-peer / dht communications actor.

/// Announce a space/agent pair on this network.
pub struct Join {
    /// The "space" context.
    pub space: super::KitsuneSpace,
    /// The "agent" context.
    pub agent: super::KitsuneAgent,
}

/// Withdraw this space/agent pair from this network.
pub struct Leave {
    /// The "space" context.
    pub space: super::KitsuneSpace,
    /// The "agent" context.
    pub agent: super::KitsuneAgent,
}

/// Make a request of a remote agent.
pub struct Request {
    /// The "space" context.
    pub space: super::KitsuneSpace,
    /// The "agent" context.
    pub agent: super::KitsuneAgent,
    /// Request data.
    pub request: Vec<u8>,
}

/// Publish data to a "neighborhood" of remote nodes surrounding the "basis" hash.
/// Returns an approximate number of nodes reached.
pub struct Broadcast {
    /// The "space" context.
    pub space: super::KitsuneSpace,
    /// The "agent" context.
    pub agent: super::KitsuneAgent,
    /// The "basis" hash/coordinate of destination neigborhood.
    pub basis: super::KitsuneBasis,
    /// The timeout to await responses - set to zero if you don't care
    /// to get a count.
    pub timeout_ms: u64,
    /// Broadcast data.
    pub broadcast: Vec<u8>,
}

/// Make a request to multiple destination agents - awaiting/aggregating the responses.
/// The remote sides will see these messages as "RequestEvt" events.
pub struct MultiRequest {
    /// The "space" context.
    pub space: super::KitsuneSpace,
    /// The "agent" context.
    pub agent: super::KitsuneAgent,
    /// The "basis" hash/coordinate of destination neigborhood.
    pub basis: super::KitsuneBasis,
    /// Target remote agent count.
    /// Set to zero for "a reasonable amount".
    /// Set to std::u32::MAX for "as many as possible".
    pub remote_agent_count: u32,
    /// The timeout to await responses.
    /// Don't set to zero - use Broadcast instead.
    pub timeout_ms: u64,
    /// Request data.
    pub request: Vec<u8>,
}

/// A response type helps indicate what agent gave what response.
pub struct MultiRequestResponse {
    /// The "agent" context.
    pub agent: super::KitsuneAgent,
    /// Response data.
    pub response: Vec<u8>,
}

ghost_actor::ghost_actor! {
    Visibility(pub),
    Name(KitsuneP2p),
    Error(super::KitsuneP2pError),
    Api {
        Join(
            "Announce a space/agent pair on this network.",
            Join,
            (),
        ),
        Leave(
            "Withdraw this space/agent pair from this network.",
            Leave,
            (),
        ),
        Request(
            "Make a request of a remote agent.",
            Request,
            Vec<u8>,
        ),
        Broadcast(
            r#"Publish data to a "neighborhood" of remote nodes surrounding the "basis" hash.
Returns an approximate number of nodes reached."#,
            Broadcast,
            u32,
        ),
        MultiRequest(
            r#"Make a request to multiple destination agents - awaiting/aggregating the responses.
The remote sides will see these messages as "RequestEvt" events."#,
            MultiRequest,
            Vec<MultiRequestResponse>,
        ),
    }
}
