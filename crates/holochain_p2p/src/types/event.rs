#![allow(clippy::too_many_arguments)]
//! Module containing incoming events from HolochainP2p.

use crate::*;
use holochain_zome_types::signature::Signature;

/// GetMeta options help control how the get is processed at various levels.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GetMetaOptions {}

impl From<&actor::GetMetaOptions> for GetMetaOptions {
    fn from(_a: &actor::GetMetaOptions) -> Self {
        Self {}
    }
}

/// GetLinks options help control how the get is processed at various levels.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GetLinksOptions {}

impl From<&actor::GetLinksOptions> for GetLinksOptions {
    fn from(_a: &actor::GetLinksOptions) -> Self {
        Self {}
    }
}

/// Get agent activity options help control how the get is processed at various levels.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GetActivityOptions {
    /// Include the activity actions in the response
    pub include_valid_activity: bool,
    /// Include any rejected actions in the response.
    pub include_rejected_activity: bool,
    /// Include warrants in the response.
    pub include_warrants: bool,
    /// Include the full records, instead of just the hashes.
    pub include_full_records: bool,
}

impl Default for GetActivityOptions {
    fn default() -> Self {
        Self {
            include_valid_activity: true,
            include_warrants: true,
            include_rejected_activity: false,
            include_full_records: false,
        }
    }
}

impl From<&actor::GetActivityOptions> for GetActivityOptions {
    fn from(a: &actor::GetActivityOptions) -> Self {
        Self {
            include_valid_activity: a.include_valid_activity,
            include_warrants: a.include_warrants,
            include_rejected_activity: a.include_rejected_activity,
            include_full_records: a.include_full_records,
        }
    }
}

/// Message between agents actively driving/negotiating a countersigning session.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CountersigningSessionNegotiationMessage {
    /// An authority has a complete set of signed actions and is responding with
    /// them back to the counterparties.
    AuthorityResponse(Vec<SignedAction>),
    /// Counterparties are sending their signed action to an enzyme instead of
    /// authorities as part of an enzymatic session.
    EnzymePush(Box<ChainOp>),
}

/// Handle requests made by remote peers.
#[cfg_attr(feature = "test_utils", automock)]
pub trait HcP2pHandler: 'static + Send + Sync + std::fmt::Debug {
    /// A remote node is attempting to make a remote call on us.
    fn handle_call_remote(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        zome_call_params_serialized: ExternIO,
        signature: Signature,
    ) -> BoxFut<'_, HolochainP2pResult<SerializedBytes>>;

    /// A remote node is publishing data in a range we claim to be holding.
    fn handle_publish(
        &self,
        dna_hash: DnaHash,
        ops: Vec<holochain_types::dht_op::DhtOp>,
    ) -> BoxFut<'_, HolochainP2pResult<()>>;

    /// A remote node is requesting entry data from us.
    fn handle_get(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        dht_hash: holo_hash::AnyDhtHash,
    ) -> BoxFut<'_, HolochainP2pResult<WireOps>>;

    /// A remote node is requesting metadata from us.
    fn handle_get_meta(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        dht_hash: holo_hash::AnyDhtHash,
        options: GetMetaOptions,
    ) -> BoxFut<'_, HolochainP2pResult<MetadataSet>>;

    /// A remote node is requesting link data from us.
    fn handle_get_links(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        link_key: WireLinkKey,
        options: GetLinksOptions,
    ) -> BoxFut<'_, HolochainP2pResult<WireLinkOps>>;

    /// A remote node is requesting a link count from us.
    fn handle_count_links(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        query: WireLinkQuery,
    ) -> BoxFut<'_, HolochainP2pResult<CountLinksResponse>>;

    /// A remote node is requesting agent activity from us.
    fn handle_get_agent_activity(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: GetActivityOptions,
    ) -> BoxFut<'_, HolochainP2pResult<AgentActivityResponse>>;

    /// A remote node is requesting agent activity from us.
    fn handle_must_get_agent_activity(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        author: AgentPubKey,
        filter: holochain_zome_types::chain::ChainFilter,
    ) -> BoxFut<'_, HolochainP2pResult<MustGetAgentActivityResponse>>;

    /// A remote node has sent us a validation receipt.
    fn handle_validation_receipts_received(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        receipts: ValidationReceiptBundle,
    ) -> BoxFut<'_, HolochainP2pResult<()>>;

    /// A remote node is publishing countersigning data to us.
    fn handle_publish_countersign(
        &self,
        dna_hash: DnaHash,
        op: holochain_types::dht_op::ChainOp,
    ) -> BoxFut<'_, HolochainP2pResult<()>>;

    /// Messages between agents that drive a countersigning session.
    fn handle_countersigning_session_negotiation(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        message: CountersigningSessionNegotiationMessage,
    ) -> BoxFut<'_, HolochainP2pResult<()>>;
}

/// Trait-object HcP2pHandler.
pub type DynHcP2pHandler = Arc<dyn HcP2pHandler>;
