//! Module containing the HolochainP2p actor definition.
#![allow(clippy::too_many_arguments)]

use crate::*;
use holochain_zome_types::request::MetadataRequest;

/// Request a validation package.
pub struct GetValidationPackage {
    /// The dna_hash / space_hash context.
    pub dna_hash: DnaHash,
    /// The agent_id / agent_pub_key context.
    pub agent_pub_key: AgentPubKey,
    // TODO - parameters
}

#[derive(Clone)]
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
}

impl Default for GetOptions {
    fn default() -> Self {
        Self {
            remote_agent_count: None,
            timeout_ms: None,
            as_race: true,
            race_timeout_ms: None,
            follow_redirects: true,
        }
    }
}

/// Get metadata from the DHT.
/// Fields tagged with `[Network]` are network-level controls.
/// Fields tagged with `[Remote]` are controls that will be forwarded to the
/// remote agent processing this `GetLinks` request.
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

ghost_actor::ghost_chan! {
    /// The HolochainP2pSender struct allows controlling the HolochainP2p
    /// actor instance.
    pub chan HolochainP2p<HolochainP2pError> {
        /// The p2p module must be informed at runtime which dna/agent pairs it should be tracking.
        fn join(dna_hash: DnaHash, agent_pub_key: AgentPubKey) -> ();

        /// If a cell is deactivated, we'll need to \"leave\" the network module as well.
        fn leave(dna_hash: DnaHash, agent_pub_key: AgentPubKey) -> ();

        /// Invoke a zome function on a remote node (if you have been granted the capability).
        fn call_remote(
            dna_hash: DnaHash,
            from_agent: AgentPubKey,
            to_agent: AgentPubKey,
            zome_name: ZomeName,
            fn_name: String,
            cap: CapSecret,
            request: SerializedBytes,
        ) -> SerializedBytes;

        /// Publish data to the correct neighborhood.
        fn publish(
            dna_hash: DnaHash,
            from_agent: AgentPubKey,
            request_validation_receipt: bool,
            dht_hash: holo_hash::AnyDhtHash,
            ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
            timeout_ms: Option<u64>,
        ) -> ();

        /// Request a validation package.
        fn get_validation_package(input: GetValidationPackage) -> (); // TODO - proper return type

        /// Get an entry from the DHT.
        fn get(
            dna_hash: DnaHash,
            from_agent: AgentPubKey,
            dht_hash: holo_hash::AnyDhtHash,
            options: GetOptions,
        ) -> Vec<GetElementResponse>;

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
            dht_hash: holo_hash::AnyDhtHash,
            options: GetLinksOptions,
        ) -> Vec<SerializedBytes>;

        /// Send a validation receipt to a remote node.
        fn send_validation_receipt(dna_hash: DnaHash, agent_pub_key: AgentPubKey, receipt: SerializedBytes) -> ();
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
