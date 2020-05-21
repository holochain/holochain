#![deny(missing_docs)]
//! P2p / dht communication framework.

/// KitsuneP2p Error Type.
#[derive(Debug, thiserror::Error)]
pub enum KitsuneP2pError {
    /// GhostError
    #[error(transparent)]
    GhostError(#[from] ghost_actor::GhostError),

    /// Custom
    #[error("Custom: {0}")]
    Custom(Box<dyn std::error::Error + Send + Sync>),

    /// Other
    #[error("Other: {0}")]
    Other(String),
}

impl KitsuneP2pError {
    /// promote a custom error type to a KitsuneP2pError
    pub fn custom(e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::Custom(e.into())
    }
}

impl From<String> for KitsuneP2pError {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

impl From<&str> for KitsuneP2pError {
    fn from(s: &str) -> Self {
        s.to_string().into()
    }
}

/// Distinguish multiple categories of communication within the same network module.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    shrinkwraprs::Shrinkwrap,
    derive_more::From,
    derive_more::Into,
)]
#[shrinkwrap(mutable)]
pub struct KitsuneSpace(pub Vec<u8>);

/// Distinguish multiple agents within the same network module.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    shrinkwraprs::Shrinkwrap,
    derive_more::From,
    derive_more::Into,
)]
#[shrinkwrap(mutable)]
pub struct KitsuneAgent(pub Vec<u8>);

/// The basis hash/coordinate when identifying a neighborhood.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    shrinkwraprs::Shrinkwrap,
    derive_more::From,
    derive_more::Into,
)]
#[shrinkwrap(mutable)]
pub struct KitsuneBasis(pub Vec<u8>);

/// The unique address of an item of distributed data accessible on the Kitsune network.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    shrinkwraprs::Shrinkwrap,
    derive_more::From,
    derive_more::Into,
)]
#[shrinkwrap(mutable)]
pub struct KitsuneDataHash(pub Vec<u8>);

/// Top-level "KitsuneAddress" items are buckets of related meta-data.
/// These "Operations" each also have unique Aspect Hashes
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    shrinkwraprs::Shrinkwrap,
    derive_more::From,
    derive_more::Into,
)]
#[shrinkwrap(mutable)]
pub struct KitsuneOpHash(pub Vec<u8>);

/// A cryptographic signature.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    shrinkwraprs::Shrinkwrap,
    derive_more::From,
    derive_more::Into,
)]
#[shrinkwrap(mutable)]
pub struct KitsuneSignature(pub Vec<u8>);

/// Definitions for events emited from the KitsuneP2p actor.
pub mod event {
    /// We are receiving a request from a remote node.
    pub struct RequestEvt {
        /// The "space" context.
        pub space: super::KitsuneSpace,
        /// The "agent" context.
        pub agent: super::KitsuneAgent,
        /// Request data.
        pub request: Vec<u8>,
    }

    /// We are receiving a broadcast from a remote node.
    pub struct BroadcastEvt {
        /// The "space" context.
        pub space: super::KitsuneSpace,
        /// The "agent" context.
        pub agent: super::KitsuneAgent,
        /// Broadcast data.
        pub broadcast: Vec<u8>,
    }

    /// Gather a list of op-hashes from our implementor that meet criteria.
    pub struct FetchOpHashesForConstraintsEvt {
        /// The "space" context.
        pub space: super::KitsuneSpace,
        /// The "agent" context.
        pub agent: super::KitsuneAgent,
        /// The start point on the dht arc to query.
        pub dht_arc_start_loc: u32,
        /// The arc-length to query.
        pub dht_arc_length: u64,
        /// If specified, only retreive items received since this time.
        pub since: Option<std::time::SystemTime>,
        /// If specified, only retreive items received until this time.
        pub until: Option<std::time::SystemTime>,
    }

    /// Gather all op-hash data for a list of op-hashes from our implementor.
    pub struct FetchOpHashDataEvt {
        /// The "space" context.
        pub space: super::KitsuneSpace,
        /// The "agent" context.
        pub agent: super::KitsuneAgent,
        /// The op-hashes to fetch
        pub op_hashes: Vec<super::KitsuneOpHash>,
    }

    /// Request that our implementor sign some data on behalf of an agent.
    pub struct SignNetworkDataEvt {
        /// The "space" context.
        pub space: super::KitsuneSpace,
        /// The "agent" context.
        pub agent: super::KitsuneAgent,
        /// The data to sign.
        pub data: Vec<u8>,
    }

    ghost_actor::ghost_chan! {
        Visibility(pub),
        Name(KitsuneP2pEvent),
        Error(super::KitsuneP2pError),
        Api {
            Request(
                "We are receiving a request from a remote node.",
                RequestEvt,
                Vec<u8>,
            ),
            Broadcast(
                "We are receiving a broadcast from a remote node.",
                BroadcastEvt,
                (),
            ),
            FetchOpHashesForConstraints(
                "Gather a list of op-hashes from our implementor that meet criteria.",
                FetchOpHashesForConstraintsEvt,
                Vec<(super::KitsuneDataHash, Vec<super::KitsuneOpHash>)>,
            ),
            FetchOpHashData(
                "Gather all op-hash data for a list of op-hashes from our implementor.",
                FetchOpHashDataEvt,
                Vec<(super::KitsuneOpHash, Vec<u8>)>,
            ),
            SignNetworkData(
                "Request that our implementor sign some data on behalf of an agent.",
                SignNetworkDataEvt,
                super::KitsuneSignature,
            ),
        }
    }

    /// Receiver type for incoming connection events.
    pub type KitsuneP2pEventReceiver = futures::channel::mpsc::Receiver<KitsuneP2pEvent>;
}

/// Definitions related to the KitsuneP2p peer-to-peer / dht communications actor.
pub mod actor {
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
}
