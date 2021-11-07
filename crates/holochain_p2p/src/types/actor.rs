//! Module containing the HolochainP2p actor definition.
#![allow(clippy::too_many_arguments)]

use crate::event::GetRequest;
use crate::*;
use holochain_types::activity::AgentActivityResponse;

/// Request a validation package.
#[derive(Clone, Debug)]
pub struct GetValidationPackage {
    /// The dna_hash / space_hash context.
    pub dna_hash: DnaHash,
    /// The agent_id / agent_pub_key context.
    pub agent_pub_key: AgentPubKey,
    /// Request the package from this agent.
    pub request_from: AgentPubKey,
    /// Request the package for this Header
    pub header_hash: HeaderHash,
}

#[derive(Clone, Debug)]
/// Get options help control how the get is processed at various levels.
/// Fields tagged with `[Network]` are network-level controls.
/// Fields tagged with `[Remote]` are controls that will be forwarded to the
/// remote agent processing this `Get` request.
pub struct GetOptions {
    /// [Network]
    /// How many remote nodes should we make requests of / aggregate.
    /// Set to `None` for a default "best-effort".
    pub remote_agent_count: Option<u8>,

    /// [Network]
    /// Timeout to await responses for aggregation.
    /// Set to `None` for a default "best-effort".
    /// Note - if all requests time-out you will receive an empty result,
    /// not a timeout error.
    pub timeout_ms: Option<u64>,

    /// [Network]
    /// We are interested in speed. If `true` and we have any results
    /// when `race_timeout_ms` is expired, those results will be returned.
    /// After `race_timeout_ms` and before `timeout_ms` the first result
    /// received will be returned.
    pub as_race: bool,

    /// [Network]
    /// See `as_race` for details.
    /// Set to `None` for a default "best-effort" race.
    pub race_timeout_ms: Option<u64>,

    /// [Remote]
    /// Whether the remote-end should follow redirects or just return the
    /// requested entry.
    pub follow_redirects: bool,

    /// [Remote]
    /// Return all live headers even if there is deletes.
    /// Useful for metadata calls.
    pub all_live_headers_with_metadata: bool,

    /// [Remote]
    /// The type of data this get request requires.
    pub request_type: GetRequest,
}

impl Default for GetOptions {
    fn default() -> Self {
        Self {
            remote_agent_count: None,
            timeout_ms: None,
            as_race: true,
            race_timeout_ms: None,
            follow_redirects: true,
            all_live_headers_with_metadata: false,
            request_type: Default::default(),
        }
    }
}

impl From<holochain_zome_types::entry::GetOptions> for GetOptions {
    fn from(_: holochain_zome_types::entry::GetOptions) -> Self {
        Self::default()
    }
}

/// Get metadata from the DHT.
/// Fields tagged with `[Network]` are network-level controls.
/// Fields tagged with `[Remote]` are controls that will be forwarded to the
/// remote agent processing this `GetLinks` request.
#[derive(Clone, Debug)]
pub struct GetMetaOptions {
    /// [Network]
    /// How many remote nodes should we make requests of / aggregate.
    /// Set to `None` for a default "best-effort".
    pub remote_agent_count: Option<u8>,

    /// [Network]
    /// Timeout to await responses for aggregation.
    /// Set to `None` for a default "best-effort".
    /// Note - if all requests time-out you will receive an empty result,
    /// not a timeout error.
    pub timeout_ms: Option<u64>,

    /// [Network]
    /// We are interested in speed. If `true` and we have any results
    /// when `race_timeout_ms` is expired, those results will be returned.
    /// After `race_timeout_ms` and before `timeout_ms` the first result
    /// received will be returned.
    pub as_race: bool,

    /// [Network]
    /// See `as_race` for details.
    /// Set to `None` for a default "best-effort" race.
    pub race_timeout_ms: Option<u64>,

    /// [Remote]
    /// Tells the remote-end which metadata to return
    pub metadata_request: MetadataRequest,
}

impl Default for GetMetaOptions {
    fn default() -> Self {
        Self {
            remote_agent_count: None,
            timeout_ms: None,
            as_race: true,
            race_timeout_ms: None,
            metadata_request: MetadataRequest::default(),
        }
    }
}

#[derive(Debug, Clone)]
/// Get links from the DHT.
/// Fields tagged with `[Network]` are network-level controls.
/// Fields tagged with `[Remote]` are controls that will be forwarded to the
/// remote agent processing this `GetLinks` request.
pub struct GetLinksOptions {
    /// [Network]
    /// Timeout to await responses for aggregation.
    /// Set to `None` for a default "best-effort".
    /// Note - if all requests time-out you will receive an empty result,
    /// not a timeout error.
    pub timeout_ms: Option<u64>,
}

impl Default for GetLinksOptions {
    fn default() -> Self {
        Self { timeout_ms: None }
    }
}

#[derive(Debug, Clone)]
/// Get agent activity from the DHT.
/// Fields tagged with `[Network]` are network-level controls.
/// Fields tagged with `[Remote]` are controls that will be forwarded to the
/// remote agent processing this `GetLinks` request.
pub struct GetActivityOptions {
    /// [Network]
    /// Timeout to await responses for aggregation.
    /// Set to `None` for a default "best-effort".
    /// Note - if all requests time-out you will receive an empty result,
    /// not a timeout error.
    pub timeout_ms: Option<u64>,
    /// Number of times to retry getting elements in parallel.
    /// For a small dht a large parallel get can overwhelm a single
    /// agent and it can be worth retrying the elements that didn't
    /// get found.
    pub retry_gets: u8,
    /// [Remote]
    /// Include the all valid activity headers in the response.
    /// If this is false the call becomes a lightweight response with
    /// just the chain status and highest observed header.
    /// This is useful when you want to ask an authority about the
    /// status of a chain but do not need all the headers.
    pub include_valid_activity: bool,
    /// Include any rejected headers in the response.
    pub include_rejected_activity: bool,
    /// Include the full signed headers and hashes in the response
    /// instead of just the hashes.
    pub include_full_headers: bool,
}

impl Default for GetActivityOptions {
    fn default() -> Self {
        Self {
            timeout_ms: None,
            retry_gets: 0,
            include_valid_activity: true,
            include_rejected_activity: false,
            include_full_headers: false,
        }
    }
}

ghost_actor::ghost_chan! {
    /// The HolochainP2pSender struct allows controlling the HolochainP2p
    /// actor instance.
    pub chan HolochainP2p<HolochainP2pError> {
        /// The p2p module must be informed at runtime which dna/agent pairs it should be tracking.
        fn join(dna_hash: DnaHash, agent_pub_key: AgentPubKey) -> ();

        /// If a cell is disabled, we'll need to \"leave\" the network module as well.
        fn leave(dna_hash: DnaHash, agent_pub_key: AgentPubKey) -> ();

        /// Invoke a zome function on a remote node (if you have been granted the capability).
        fn call_remote(
            dna_hash: DnaHash,
            from_agent: AgentPubKey,
            to_agent: AgentPubKey,
            zome_name: ZomeName,
            fn_name: FunctionName,
            cap_secret: Option<CapSecret>,
            payload: ExternIO,
        ) -> SerializedBytes;

        /// Invoke a zome function on a remote node (if you have been granted the capability).
        /// This is a fire-and-forget operation, a best effort will be made
        /// to forward the signal, but if the conductor network is overworked
        /// it may decide not to deliver some of the signals.
        fn remote_signal(
            dna_hash: DnaHash,
            from_agent: AgentPubKey,
            to_agent_list: Vec<AgentPubKey>,
            zome_name: ZomeName,
            fn_name: FunctionName,
            cap: Option<CapSecret>,
            payload: ExternIO,
        ) -> ();

        /// Publish data to the correct neighborhood.
        fn publish(
            dna_hash: DnaHash,
            from_agent: AgentPubKey,
            request_validation_receipt: bool,
            countersigning_session: bool,
            dht_hash: holo_hash::AnyDhtHash,
            ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
            timeout_ms: Option<u64>,
        ) -> ();

        /// Request a validation package.
        fn get_validation_package(input: GetValidationPackage) -> ValidationPackageResponse;

        /// Get an entry from the DHT.
        fn get(
            dna_hash: DnaHash,
            from_agent: AgentPubKey,
            dht_hash: holo_hash::AnyDhtHash,
            options: GetOptions,
        ) -> Vec<WireOps>;

        /// Get metadata from the DHT.
        fn get_meta(
            dna_hash: DnaHash,
            from_agent: AgentPubKey,
            dht_hash: holo_hash::AnyDhtHash,
            options: GetMetaOptions,
        ) -> Vec<MetadataSet>;

        /// Get links from the DHT.
        fn get_links(
            dna_hash: DnaHash,
            from_agent: AgentPubKey,
            link_key: WireLinkKey,
            options: GetLinksOptions,
        ) -> Vec<WireLinkOps>;

        /// Get agent activity from the DHT.
        fn get_agent_activity(
            dna_hash: DnaHash,
            from_agent: AgentPubKey,
            agent: AgentPubKey,
            query: ChainQueryFilter,
            options: GetActivityOptions,
        ) -> Vec<AgentActivityResponse<HeaderHash>>;

        /// Send a validation receipt to a remote node.
        fn send_validation_receipt(dna_hash: DnaHash, to_agent: AgentPubKey, from_agent: AgentPubKey, receipt: SerializedBytes) -> ();

        /// New data has been integrated and is ready for gossiping.
        fn new_integrated_data(dna_hash: DnaHash) -> ();

        /// Check if an agent is an authority for a hash.
        fn authority_for_hash(dna_hash: DnaHash, from_agent: AgentPubKey, dht_hash: AnyDhtHash) -> bool;

        /// Response from an authority to agents that are
        /// part of a session.
        fn countersigning_authority_response(
            dna_hash: DnaHash,
            from_agent: AgentPubKey,
            agents: Vec<AgentPubKey>,
            signed_headers: Vec<SignedHeader>,
        ) -> ();
    }
}

/// Convenience type for referring to the HolochainP2p GhostSender
pub type HolochainP2pRef = ghost_actor::GhostSender<HolochainP2p>;

/// Extension trait for converting GhostSender<HolochainP2p> into HolochainP2pCell
pub trait HolochainP2pRefToCell {
    /// Partially apply dna_hash && agent_pub_key to this sender,
    /// binding it to a specific cell context.
    fn into_cell(self, dna_hash: DnaHash, from_agent: AgentPubKey) -> crate::HolochainP2pCell;

    /// Clone and partially apply dna_hash && agent_pub_key to this sender,
    /// binding it to a specific cell context.
    fn to_cell(&self, dna_hash: DnaHash, from_agent: AgentPubKey) -> crate::HolochainP2pCell;
}

impl HolochainP2pRefToCell for HolochainP2pRef {
    fn into_cell(self, dna_hash: DnaHash, from_agent: AgentPubKey) -> crate::HolochainP2pCell {
        crate::HolochainP2pCell {
            sender: self,
            dna_hash: Arc::new(dna_hash),
            from_agent: Arc::new(from_agent),
        }
    }

    fn to_cell(&self, dna_hash: DnaHash, from_agent: AgentPubKey) -> crate::HolochainP2pCell {
        self.clone().into_cell(dna_hash, from_agent)
    }
}
