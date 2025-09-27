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

mod peer_meta_store;
pub use peer_meta_store::*;

mod local_agent;
pub use local_agent::*;

mod op_store;
pub use op_store::*;

mod hc_report;
pub use hc_report::*;

mod blocks;
pub use blocks::*;

mod metrics;

fn check_k2_init() {
    static K2_CONFIG: std::sync::Once = std::sync::Once::new();
    K2_CONFIG.call_once(|| {
        // Set up some kitsune2 specializations specific to holochain.

        kitsune2_api::OpId::set_loc_callback(|bytes| {
            u32::from_le_bytes(
                bytes[HOLO_HASH_CORE_LEN..HOLO_HASH_UNTYPED_LEN]
                    .try_into()
                    .unwrap(),
            )
        });

        // Kitsune2 by default just xors subsequent bytes of the hash
        // itself and treats that result as a LE u32.
        // Holochain, instead, first does a blake2b hash, and
        // then xors those bytes.
        kitsune2_api::Id::set_global_loc_callback(|bytes| {
            let hash = blake2b_simd::Params::new().hash_length(16).hash(bytes);
            let hash = hash.as_bytes();
            let mut out = [hash[0], hash[1], hash[2], hash[3]];
            for i in (4..16).step_by(4) {
                out[0] ^= hash[i];
                out[1] ^= hash[i + 1];
                out[2] ^= hash[i + 2];
                out[3] ^= hash[i + 3];
            }
            u32::from_le_bytes(out)
        });

        // Kitsune2 just displays the bytes as direct base64.
        // Holochain prepends some prefix bytes and appends the loc bytes.
        kitsune2_api::SpaceId::set_global_display_callback(|bytes, f| {
            write!(f, "{}", DnaHash::from_raw_32(bytes.to_vec()))
        });
        kitsune2_api::AgentId::set_global_display_callback(|bytes, f| {
            write!(f, "{}", AgentPubKey::from_raw_32(bytes.to_vec()))
        });
        kitsune2_api::OpId::set_global_display_callback(|bytes, f| {
            write!(f, "{}", DhtOpHash::from_raw_32(bytes[0..32].to_vec()))
        });
    });
}

/// A wrapper around HolochainP2pSender that partially applies the dna_hash / agent_pub_key.
/// I.e. a sender that is tied to a specific cell.
#[automock]
#[allow(clippy::too_many_arguments)]
#[async_trait::async_trait]
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
        basis_hash: holo_hash::OpBasis,
        source: AgentPubKey,
        op_hash_list: Vec<DhtOpHash>,
        timeout_ms: Option<u64>,
        reflect_ops: Option<Vec<DhtOp>>,
    ) -> HolochainP2pResult<()>;

    /// Publish a countersigning op.
    async fn publish_countersign(
        &self,
        basis_hash: holo_hash::OpBasis,
        op: ChainOp,
    ) -> HolochainP2pResult<()>;

    /// Get an entry from the DHT.
    async fn get(&self, dht_hash: holo_hash::AnyDhtHash) -> HolochainP2pResult<Vec<WireOps>>;

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

    /// Send a validation receipt to a remote node.
    async fn send_validation_receipts(
        &self,
        to_agent: AgentPubKey,
        receipts: ValidationReceiptBundle,
    ) -> HolochainP2pResult<()>;

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

/// Trait object for HolochainP2pDnaT.
pub type DynHolochainP2pDna = Arc<dyn HolochainP2pDnaT>;

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
        basis_hash: holo_hash::OpBasis,
        op: ChainOp,
    ) -> HolochainP2pResult<()> {
        self.sender
            .publish_countersign((*self.dna_hash).clone(), basis_hash, op)
            .await
    }

    /// Get [`ChainOp::StoreRecord`] or [`ChainOp::StoreEntry`] from the DHT.
    async fn get(&self, dht_hash: holo_hash::AnyDhtHash) -> HolochainP2pResult<Vec<WireOps>> {
        self.sender
            .get((*self.dna_hash).clone(), dht_hash)
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

    /// Send a validation receipt to a remote node.
    async fn send_validation_receipts(
        &self,
        to_agent: AgentPubKey,
        receipts: ValidationReceiptBundle,
    ) -> HolochainP2pResult<()> {
        self.sender
            .send_validation_receipts((*self.dna_hash).clone(), to_agent, receipts)
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
