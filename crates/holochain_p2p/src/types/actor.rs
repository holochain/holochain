//! Module containing the HolochainP2p actor definition.
#![allow(clippy::too_many_arguments)]

use crate::*;
use holochain_types::activity::AgentActivityResponse;
use holochain_types::prelude::ValidationReceiptBundle;
use kitsune2_api::{DhtArc, SpaceId, StoredOp};
use std::collections::HashMap;

/// Get options used to control how data fetching over the network is performed.
#[derive(Clone, Debug)]
pub struct NetworkRequestOptions {
    /// Make requests to this number of remote agents in parallel.
    ///
    /// When `GetOptions::as_race` is `true`, the first response received will be returned.
    /// When `GetOptions::as_race` is `false`, responses will be aggregated until the timeout is
    /// reached. This is not implemented.
    ///
    /// Defaults to `3`.
    pub remote_agent_count: u8,

    /// Timeout within which responses must arrive.
    ///
    /// When `None` is specified, the conductor settings will be used to determine the timeout.
    pub timeout_ms: Option<u64>,

    /// Whether to treat the get as a race, returning the first response received.
    ///
    /// Defaults to `true`.
    pub as_race: bool,
}

impl Default for NetworkRequestOptions {
    fn default() -> Self {
        Self {
            remote_agent_count: 3,
            timeout_ms: None,
            as_race: true,
        }
    }
}

impl NetworkRequestOptions {
    /// Using defaults is dangerous in a must_get as it can undermine determinism.
    /// We want refactors to explicitly consider this.
    pub fn must_get_options() -> Self {
        Self {
            remote_agent_count: 1,
            timeout_ms: None,
            as_race: true,
        }
    }
}

/// Options for getting links from the network.
#[derive(Debug, Clone, Default)]
pub struct GetLinksRequestOptions {
    /// The base network options to use for this call.
    pub network_req_options: NetworkRequestOptions,

    /// Whether to fetch links from the network or return only
    /// locally available links. Defaults to fetching links from network.
    pub get_options: holochain_zome_types::entry::GetOptions,
}

/// Get agent activity from the DHT.
///
/// Fields tagged with `[Network]` are network-level controls.
/// Fields tagged with `[Remote]` are controls that will be forwarded to the
/// remote agent processing this `GetLinks` request.
#[derive(Debug, Clone)]
pub struct GetActivityOptions {
    /// The base network options to use for this call.
    pub network_req_options: NetworkRequestOptions,
    /// `[Remote]`
    /// Include the all valid activity actions in the response.
    /// If this is false the call becomes a lightweight response with
    /// just the chain status and highest observed action.
    /// This is useful when you want to ask an authority about the
    /// status of a chain but do not need all the actions.
    pub include_valid_activity: bool,
    /// Include any rejected actions in the response.
    pub include_rejected_activity: bool,
    /// Include warrants for this agent
    pub include_warrants: bool,
    /// Include the full signed records in the response, instead of just the hashes.
    pub include_full_records: bool,
    /// Configure how the data should be fetched.
    pub get_options: GetOptions,
}

impl Default for GetActivityOptions {
    fn default() -> Self {
        Self {
            network_req_options: NetworkRequestOptions::default(),
            include_valid_activity: true,
            include_rejected_activity: false,
            include_warrants: true,
            include_full_records: false,
            get_options: Default::default(),
        }
    }
}

/// Trait defining the main holochain_p2p interface.
#[cfg_attr(feature = "test_utils", automock)]
pub trait HcP2p: 'static + Send + Sync + std::fmt::Debug {
    /// Test access to underlying kitsune instance.
    #[cfg(feature = "test_utils")]
    fn test_kitsune(&self) -> &kitsune2_api::DynKitsune;

    /// Test utility to force local agents to report full storage arcs.
    #[cfg(feature = "test_utils")]
    fn test_set_full_arcs(&self, space: SpaceId) -> BoxFut<'_, ()> {
        Box::pin(async move {
            let mut updated_agents = Vec::new();
            for agent in self
                .test_kitsune()
                .space(space.clone())
                .await
                .unwrap()
                .local_agent_store()
                .get_all()
                .await
                .unwrap()
            {
                agent.set_cur_storage_arc(DhtArc::FULL);
                agent.set_tgt_storage_arc_hint(DhtArc::FULL);
                agent.invoke_cb();
                updated_agents.push(agent);
            }

            // Wait for the updated agent infos to have been put in the peer store.
            retry_fn_until_timeout(
                || {
                    let space = space.clone();
                    let updated_agents = updated_agents.clone();
                    async move {
                        let all_agents_in_peer_store = self
                            .test_kitsune()
                            .space(space.clone())
                            .await
                            .unwrap()
                            .peer_store()
                            .get_all()
                            .await
                            .unwrap();
                        updated_agents.into_iter().all(|updated_agent| {
                            all_agents_in_peer_store.iter().any(|a| {
                                a.agent == *updated_agent.agent() && a.storage_arc == DhtArc::FULL
                            })
                        })
                    }
                },
                Some(10_000),
                None,
            )
            .await
            .expect("peer store not updated after declaring full arc");
        })
    }

    /// Access the k2 peer store for a particular dna hash.
    fn peer_store(
        &self,
        dna_hash: DnaHash,
    ) -> BoxFut<'_, HolochainP2pResult<kitsune2_api::DynPeerStore>>;

    /// Call this exactly once before any other invocations on this
    /// instance in order to register the HcP2pHandler.
    fn register_handler(
        &self,
        handler: event::DynHcP2pHandler,
    ) -> BoxFut<'_, HolochainP2pResult<()>>;

    /// The p2p module must be informed at runtime which dna/agent pairs it should be tracking.
    fn join(
        &self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
        maybe_agent_info: Option<AgentInfoSigned>,
    ) -> BoxFut<'_, HolochainP2pResult<()>>;

    /// If a cell is disabled, we'll need to \"leave\" the network module as well.
    fn leave(
        &self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
    ) -> BoxFut<'_, HolochainP2pResult<()>>;

    /// Inform p2p module when ops have been integrated into the store, so that it can start
    /// gossiping them.
    fn new_integrated_data(
        &self,
        space_id: SpaceId,
        ops: Vec<StoredOp>,
    ) -> BoxFut<'_, HolochainP2pResult<()>>;

    /// Invoke a zome function on a remote node (if you have been granted the capability).
    fn call_remote(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        zome_call_params_serialized: ExternIO,
        signature: Signature,
    ) -> BoxFut<'_, HolochainP2pResult<SerializedBytes>>;

    /// Invoke a zome function on a remote node (if you have been granted the capability).
    /// This is a fire-and-forget operation, a best effort will be made
    /// to forward the signal, but if the conductor network is overworked
    /// it may decide not to deliver some of the signals.
    fn send_remote_signal(
        &self,
        dna_hash: DnaHash,
        to_agent_list: Vec<(AgentPubKey, ExternIO, Signature)>,
    ) -> BoxFut<'_, HolochainP2pResult<()>>;

    /// Publish data to the correct neighborhood.
    fn publish(
        &self,
        dna_hash: DnaHash,
        basis_hash: OpBasis,
        source: AgentPubKey,
        op_hash_list: Vec<DhtOpHash>,
        timeout_ms: Option<u64>,
        reflect_ops: Option<Vec<DhtOp>>,
    ) -> BoxFut<'_, HolochainP2pResult<()>>;

    /// Publish a countersigning op.
    fn publish_countersign(
        &self,
        dna_hash: DnaHash,
        basis_hash: OpBasis,
        op: ChainOp,
    ) -> BoxFut<'_, HolochainP2pResult<()>>;

    /// Get an entry from the DHT.
    fn get(
        &self,
        dna_hash: DnaHash,
        dht_hash: AnyDhtHash,
        options: NetworkRequestOptions,
    ) -> BoxFut<'_, HolochainP2pResult<Vec<WireOps>>>;

    /// Get links from the DHT.
    fn get_links(
        &self,
        dna_hash: DnaHash,
        link_key: WireLinkKey,
        options: GetLinksRequestOptions,
    ) -> BoxFut<'_, HolochainP2pResult<Vec<WireLinkOps>>>;

    /// Get a count of links from the DHT.
    fn count_links(
        &self,
        dna_hash: DnaHash,
        query: WireLinkQuery,
        options: NetworkRequestOptions,
    ) -> BoxFut<'_, HolochainP2pResult<CountLinksResponse>>;

    /// Get agent activity from the DHT.
    fn get_agent_activity(
        &self,
        dna_hash: DnaHash,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: GetActivityOptions,
    ) -> BoxFut<'_, HolochainP2pResult<Vec<AgentActivityResponse>>>;

    /// A remote node is requesting agent activity from us.
    fn must_get_agent_activity(
        &self,
        dna_hash: DnaHash,
        author: AgentPubKey,
        filter: holochain_zome_types::chain::ChainFilter,
        options: NetworkRequestOptions,
    ) -> BoxFut<'_, HolochainP2pResult<Vec<MustGetAgentActivityResponse>>>;

    /// Send a validation receipt to a remote node.
    fn send_validation_receipts(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        receipts: ValidationReceiptBundle,
    ) -> BoxFut<'_, HolochainP2pResult<()>>;

    /// Check if any local agent in this space is an authority for a hash.
    fn authority_for_hash(
        &self,
        dna_hash: DnaHash,
        basis: OpBasis,
    ) -> BoxFut<'_, HolochainP2pResult<bool>>;

    /// Messages between agents negotiation a countersigning session.
    fn countersigning_session_negotiation(
        &self,
        dna_hash: DnaHash,
        agents: Vec<AgentPubKey>,
        message: event::CountersigningSessionNegotiationMessage,
    ) -> BoxFut<'_, HolochainP2pResult<()>>;

    /// Dump network metrics.
    fn dump_network_metrics(
        &self,
        request: Kitsune2NetworkMetricsRequest,
    ) -> BoxFut<'_, HolochainP2pResult<HashMap<DnaHash, Kitsune2NetworkMetrics>>>;

    /// Get network stats from the Kitsune2 transport.
    ///
    /// See [Transport::dump_network_stats](kitsune2_api::Transport::dump_network_stats).
    fn dump_network_stats(&self)
        -> BoxFut<'_, HolochainP2pResult<kitsune2_api::ApiTransportStats>>;

    /// Get the target arcs of the agents currently in this space.
    fn target_arcs(
        &self,
        dna_hash: DnaHash,
    ) -> BoxFut<'_, HolochainP2pResult<Vec<kitsune2_api::DhtArc>>>;

    /// Block an agent for a DNA with a block reason.
    fn block(&self, block: Block) -> BoxFut<'_, HolochainP2pResult<()>>;

    /// Query if an agent is blocked.
    fn is_blocked(&self, target: BlockTargetId) -> BoxFut<'_, HolochainP2pResult<bool>>;

    /// Get the conductor database getter.
    fn conductor_db_getter(&self) -> crate::GetDbConductor;
}

/// Trait-object HcP2p
pub type DynHcP2p = Arc<dyn HcP2p>;
