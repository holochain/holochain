#![deny(missing_docs)]
//! holochain specific wrapper around more generic p2p module

use holo_hash::*;
use holochain_chc::ChcImpl;
use holochain_serialized_bytes::prelude::*;
use holochain_types::prelude::*;
use kitsune2_api::{AgentInfoSigned, BoxFut};
use kitsune2_api::{SpaceId, StoredOp};
use mockall::automock;
use std::sync::Arc;
use tracing::Instrument;

mod types;
pub use types::*;

mod spawn;
pub use spawn::*;
#[cfg(feature = "test_utils")]
pub use test::stub_network;
#[cfg(feature = "test_utils")]
pub use test::HolochainP2pDnaFixturator;

mod peer_meta_store;
pub use peer_meta_store::*;

mod local_agent;
pub use local_agent::*;

mod op_store;
pub use op_store::*;

#[automock]
#[allow(clippy::too_many_arguments)]
#[async_trait::async_trait]
/// A wrapper around HolochainP2pSender that partially applies the dna_hash / agent_pub_key.
/// I.e. a sender that is tied to a specific cell.
pub trait HolochainP2pDnaT: Send + Sync + 'static {
    /// owned getter
    fn dna_hash(&self) -> DnaHash;

    /// The p2p module must be informed at runtime which dna/agent pairs it should be tracking.
    async fn join(
        &self,
        agent: AgentPubKey,
        maybe_agent_info: Option<AgentInfoSigned>,
    ) -> HolochainP2pResult<()>;

    /// If a cell is disabled, we'll need to \"leave\" the network module as well.
    async fn leave(&self, agent: AgentPubKey) -> HolochainP2pResult<()>;

    /// Inform p2p module when ops have been integrated into the store, so that it can start
    /// gossiping them.
    async fn new_integrated_data(&self, ops: Vec<StoredOp>) -> HolochainP2pResult<()>;

    /// Invoke a zome function on a remote node (if you have been granted the capability).
    #[allow(clippy::too_many_arguments)]
    async fn call_remote(
        &self,
        to_agent: AgentPubKey,
        zome_call_params_serialized: ExternIO,
        signature: Signature,
    ) -> HolochainP2pResult<SerializedBytes>;

    /// Invoke a zome function on a remote node (if you have been granted the capability).
    /// This is a fire-and-forget operation, a best effort will be made
    /// to forward the signal, but if the conductor network is overworked
    /// it may decide not to deliver some of the signals.
    async fn send_remote_signal(
        &self,
        to_agent_list: Vec<(AgentPubKey, ExternIO, Signature)>,
    ) -> HolochainP2pResult<()>;

    /// Publish data to the correct neighborhood.
    #[allow(clippy::ptr_arg)]
    async fn publish(
        &self,
        request_validation_receipt: bool,
        countersigning_session: bool,
        basis_hash: holo_hash::OpBasis,
        source: AgentPubKey,
        op_hash_list: Vec<DhtOpHash>,
        timeout_ms: Option<u64>,
        reflect_ops: Option<Vec<DhtOp>>,
    ) -> HolochainP2pResult<()>;

    /// Publish a countersigning op.
    async fn publish_countersign(
        &self,
        flag: bool,
        basis_hash: holo_hash::OpBasis,
        op: DhtOp,
    ) -> HolochainP2pResult<()>;

    /// Get an entry from the DHT.
    async fn get(
        &self,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetOptions,
    ) -> HolochainP2pResult<Vec<WireOps>>;

    /// Get metadata from the DHT.
    async fn get_meta(
        &self,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetMetaOptions,
    ) -> HolochainP2pResult<Vec<MetadataSet>>;

    /// Get links from the DHT.
    async fn get_links(
        &self,
        link_key: WireLinkKey,
        options: actor::GetLinksOptions,
    ) -> HolochainP2pResult<Vec<WireLinkOps>>;

    /// Get a count of links from the DHT.
    async fn count_links(&self, query: WireLinkQuery) -> HolochainP2pResult<CountLinksResponse>;

    /// Get agent activity from the DHT.
    async fn get_agent_activity(
        &self,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: actor::GetActivityOptions,
    ) -> HolochainP2pResult<Vec<AgentActivityResponse>>;

    /// Get agent activity deterministically from the DHT.
    async fn must_get_agent_activity(
        &self,
        author: AgentPubKey,
        filter: holochain_zome_types::chain::ChainFilter,
    ) -> HolochainP2pResult<Vec<MustGetAgentActivityResponse>>;

    /// Request validation receipts for op hashes from remote nodes
    /// (excluding those we already have).
    async fn get_validation_receipts(
        &self,
        basis_hash: holo_hash::OpBasis,
        op_hash_list: Vec<DhtOpHash>,
        exclude_list: Vec<AgentPubKey>,
        limit: usize,
    ) -> HolochainP2pResult<ValidationReceiptBundle>;

    /// Check if an agent is an authority for a hash.
    async fn authority_for_hash(&self, basis: holo_hash::OpBasis) -> HolochainP2pResult<bool>;

    /// Messages between agents driving a countersigning session.
    async fn countersigning_session_negotiation(
        &self,
        agents: Vec<AgentPubKey>,
        message: event::CountersigningSessionNegotiationMessage,
    ) -> HolochainP2pResult<()>;

    /// Get the target arcs of the agents currently in this space.
    async fn target_arcs(&self) -> HolochainP2pResult<Vec<kitsune2_api::DhtArc>>;

    /// Access to the specified CHC
    fn chc(&self) -> Option<ChcImpl>;
}

/// A wrapper around HolochainP2pSender that partially applies the dna_hash / agent_pub_key.
/// I.e. a sender that is tied to a specific cell.
#[derive(Clone)]
pub struct HolochainP2pDna {
    sender: actor::DynHcP2p,
    dna_hash: Arc<DnaHash>,
    chc: Option<ChcImpl>,
}

impl HolochainP2pDna {
    /// Construct a HolochainP2pDna from components.
    pub fn new(hc_p2p: actor::DynHcP2p, dna_hash: DnaHash, chc: Option<ChcImpl>) -> Self {
        Self {
            sender: hc_p2p,
            dna_hash: dna_hash.into(),
            chc,
        }
    }
}

impl From<HolochainP2pDna> for GenericNetwork {
    fn from(value: HolochainP2pDna) -> Self {
        Arc::new(value)
    }
}

#[async_trait::async_trait]
impl HolochainP2pDnaT for HolochainP2pDna {
    /// owned getter
    fn dna_hash(&self) -> DnaHash {
        (*self.dna_hash).clone()
    }

    /// The p2p module must be informed at runtime which dna/agent pairs it should be tracking.
    async fn join(
        &self,
        agent: AgentPubKey,
        maybe_agent_info: Option<AgentInfoSigned>,
    ) -> HolochainP2pResult<()> {
        self.sender
            .join((*self.dna_hash).clone(), agent, maybe_agent_info)
            .await
    }

    /// If a cell is disabled, we'll need to \"leave\" the network module as well.
    async fn leave(&self, agent: AgentPubKey) -> HolochainP2pResult<()> {
        self.sender.leave((*self.dna_hash).clone(), agent).await
    }

    /// Inform p2p module when ops have been integrated into the store, so that it can start
    /// gossiping them.
    async fn new_integrated_data(&self, ops: Vec<StoredOp>) -> HolochainP2pResult<()> {
        self.sender
            .new_integrated_data(self.dna_hash.to_k2_space(), ops)
            .await
    }

    /// Invoke a zome function on a remote node (if you have been granted the capability).
    async fn call_remote(
        &self,
        to_agent: AgentPubKey,
        zome_call_params_serialized: ExternIO,
        signature: Signature,
    ) -> HolochainP2pResult<SerializedBytes> {
        self.sender
            .call_remote(
                (*self.dna_hash).clone(),
                to_agent,
                zome_call_params_serialized,
                signature,
            )
            .await
    }

    /// Invoke a zome function on a remote node (if you have been granted the capability).
    /// This is a fire-and-forget operation, a best effort will be made
    /// to forward the signal, but if the conductor network is overworked
    /// it may decide not to deliver some of the signals.
    async fn send_remote_signal(
        &self,
        to_agent_list: Vec<(AgentPubKey, ExternIO, Signature)>,
    ) -> HolochainP2pResult<()> {
        self.sender
            .send_remote_signal((*self.dna_hash).clone(), to_agent_list)
            .await
    }

    /// Publish data to the correct neighborhood.
    async fn publish(
        &self,
        request_validation_receipt: bool,
        countersigning_session: bool,
        basis_hash: holo_hash::OpBasis,
        source: AgentPubKey,
        op_hash_list: Vec<DhtOpHash>,
        timeout_ms: Option<u64>,
        reflect_ops: Option<Vec<DhtOp>>,
    ) -> HolochainP2pResult<()> {
        self.sender
            .publish(
                (*self.dna_hash).clone(),
                request_validation_receipt,
                countersigning_session,
                basis_hash,
                source,
                op_hash_list,
                timeout_ms,
                reflect_ops,
            )
            .await
    }

    /// Publish a countersigning op.
    async fn publish_countersign(
        &self,
        flag: bool,
        basis_hash: holo_hash::OpBasis,
        op: DhtOp,
    ) -> HolochainP2pResult<()> {
        self.sender
            .publish_countersign((*self.dna_hash).clone(), flag, basis_hash, op)
            .await
    }

    /// Get [`ChainOp::StoreRecord`] or [`ChainOp::StoreEntry`] from the DHT.
    async fn get(
        &self,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetOptions,
    ) -> HolochainP2pResult<Vec<WireOps>> {
        self.sender
            .get((*self.dna_hash).clone(), dht_hash, options)
            .instrument(tracing::debug_span!("HolochainP2p::get"))
            .await
    }

    /// Get metadata from the DHT.
    async fn get_meta(
        &self,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetMetaOptions,
    ) -> HolochainP2pResult<Vec<MetadataSet>> {
        self.sender
            .get_meta((*self.dna_hash).clone(), dht_hash, options)
            .await
    }

    /// Get links from the DHT.
    async fn get_links(
        &self,
        link_key: WireLinkKey,
        options: actor::GetLinksOptions,
    ) -> HolochainP2pResult<Vec<WireLinkOps>> {
        self.sender
            .get_links((*self.dna_hash).clone(), link_key, options)
            .await
    }

    /// Get a count of links from the DHT.
    async fn count_links(&self, query: WireLinkQuery) -> HolochainP2pResult<CountLinksResponse> {
        self.sender
            .count_links((*self.dna_hash).clone(), query)
            .await
    }

    /// Get agent activity from the DHT.
    async fn get_agent_activity(
        &self,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: actor::GetActivityOptions,
    ) -> HolochainP2pResult<Vec<AgentActivityResponse>> {
        self.sender
            .get_agent_activity((*self.dna_hash).clone(), agent, query, options)
            .await
    }

    async fn must_get_agent_activity(
        &self,
        author: AgentPubKey,
        filter: holochain_zome_types::chain::ChainFilter,
    ) -> HolochainP2pResult<Vec<MustGetAgentActivityResponse>> {
        self.sender
            .must_get_agent_activity((*self.dna_hash).clone(), author, filter)
            .await
    }

    async fn get_validation_receipts(
        &self,
        basis_hash: holo_hash::OpBasis,
        op_hash_list: Vec<DhtOpHash>,
        exclude_list: Vec<AgentPubKey>,
        limit: usize,
    ) -> HolochainP2pResult<ValidationReceiptBundle> {
        self.sender
            .get_validation_receipts(
                (*self.dna_hash).clone(),
                basis_hash,
                op_hash_list,
                exclude_list,
                limit,
            )
            .await
    }

    /// Check if an agent is an authority for a hash.
    async fn authority_for_hash(&self, dht_hash: holo_hash::OpBasis) -> HolochainP2pResult<bool> {
        self.sender
            .authority_for_hash((*self.dna_hash).clone(), dht_hash)
            .await
    }

    async fn countersigning_session_negotiation(
        &self,
        agents: Vec<AgentPubKey>,
        message: event::CountersigningSessionNegotiationMessage,
    ) -> HolochainP2pResult<()> {
        self.sender
            .countersigning_session_negotiation((*self.dna_hash).clone(), agents, message)
            .await
    }

    async fn target_arcs(&self) -> HolochainP2pResult<Vec<kitsune2_api::DhtArc>> {
        self.sender.target_arcs((*self.dna_hash).clone()).await
    }

    fn chc(&self) -> Option<ChcImpl> {
        self.chc.clone()
    }
}

#[allow(unused)]
#[cfg(any(test, feature = "test_utils"))]
mod test;
