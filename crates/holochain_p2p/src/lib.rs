#![deny(missing_docs)]
//! holochain specific wrapper around more generic p2p module

use holo_hash::*;
use holochain_serialized_bytes::prelude::*;
use holochain_types::prelude::*;
use std::sync::Arc;

mod types;
pub use types::actor::HolochainP2pRef;
pub use types::actor::HolochainP2pSender;
pub use types::AgentPubKeyExt; // why is this not included by * above???
pub use types::*;

mod spawn;
use ghost_actor::dependencies::tracing;
use ghost_actor::dependencies::tracing_futures::Instrument;
pub use spawn::*;
pub use test::stub_network;
pub use test::HolochainP2pCellFixturator;

pub use kitsune_p2p;

#[mockall::automock]
#[async_trait::async_trait]
/// A wrapper around HolochainP2pSender that partially applies the dna_hash / agent_pub_key.
/// I.e. a sender that is tied to a specific cell.
pub trait HolochainP2pCellT {
    /// owned getter
    fn dna_hash(&self) -> DnaHash;

    /// owned getter
    fn from_agent(&self) -> AgentPubKey;

    /// Construct the CellId from the defined DnaHash and AgentPubKey
    fn cell_id(&self) -> CellId {
        CellId::new(self.dna_hash(), self.from_agent())
    }

    /// The p2p module must be informed at runtime which dna/agent pairs it should be tracking.
    async fn join(&self) -> actor::HolochainP2pResult<()>;

    /// If a cell is disabled, we'll need to \"leave\" the network module as well.
    async fn leave(&self) -> actor::HolochainP2pResult<()>;

    /// Invoke a zome function on a remote node (if you have been granted the capability).
    async fn call_remote(
        &self,
        to_agent: AgentPubKey,
        zome_name: ZomeName,
        fn_name: FunctionName,
        cap: Option<CapSecret>,
        payload: ExternIO,
    ) -> actor::HolochainP2pResult<SerializedBytes>;

    /// Publish data to the correct neighborhood.
    #[allow(clippy::ptr_arg)]
    async fn publish(
        &self,
        request_validation_receipt: bool,
        countersigning_session: bool,
        dht_hash: holo_hash::AnyDhtHash,
        ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
        timeout_ms: Option<u64>,
    ) -> actor::HolochainP2pResult<()>;

    /// Request a validation package.
    async fn get_validation_package(
        &self,
        request_from: AgentPubKey,
        header_hash: HeaderHash,
    ) -> actor::HolochainP2pResult<ValidationPackageResponse>;

    /// Get an entry from the DHT.
    async fn get(
        &self,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetOptions,
    ) -> actor::HolochainP2pResult<Vec<WireOps>>;

    /// Get metadata from the DHT.
    async fn get_meta(
        &self,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetMetaOptions,
    ) -> actor::HolochainP2pResult<Vec<MetadataSet>>;

    /// Get links from the DHT.
    async fn get_links(
        &self,
        link_key: WireLinkKey,
        options: actor::GetLinksOptions,
    ) -> actor::HolochainP2pResult<Vec<WireLinkOps>>;

    /// Get agent activity from the DHT.
    async fn get_agent_activity(
        &self,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: actor::GetActivityOptions,
    ) -> actor::HolochainP2pResult<Vec<AgentActivityResponse<HeaderHash>>>;

    /// Send a validation receipt to a remote node.
    async fn send_validation_receipt(
        &self,
        to_agent: AgentPubKey,
        receipt: SerializedBytes,
    ) -> actor::HolochainP2pResult<()>;

    /// Check if an agent is an authority for a hash.
    async fn authority_for_hash(
        &self,
        dht_hash: holo_hash::AnyDhtHash,
    ) -> actor::HolochainP2pResult<bool>;

    /// Response from an authority to agents that are
    /// part of a session.
    async fn countersigning_authority_response(
        &self,
        agents: Vec<AgentPubKey>,
        response: Vec<SignedHeader>,
    ) -> actor::HolochainP2pResult<()>;

    /// New data has been integrated and is ready for gossiping.
    async fn new_integrated_data(&self, authored: bool) -> actor::HolochainP2pResult<()>;
}

/// A wrapper around HolochainP2pSender that partially applies the dna_hash / agent_pub_key.
/// I.e. a sender that is tied to a specific cell.
#[derive(Clone)]
pub struct HolochainP2pCell {
    sender: ghost_actor::GhostSender<actor::HolochainP2p>,
    dna_hash: Arc<DnaHash>,
    from_agent: Arc<AgentPubKey>,
}

#[async_trait::async_trait]
impl HolochainP2pCellT for HolochainP2pCell {
    /// owned getter
    fn dna_hash(&self) -> DnaHash {
        (*self.dna_hash).clone()
    }

    /// owned getter
    fn from_agent(&self) -> AgentPubKey {
        (*self.from_agent).clone()
    }

    /// The p2p module must be informed at runtime which dna/agent pairs it should be tracking.
    async fn join(&self) -> actor::HolochainP2pResult<()> {
        self.sender
            .join((*self.dna_hash).clone(), (*self.from_agent).clone())
            .await
    }

    /// If a cell is disabled, we'll need to \"leave\" the network module as well.
    async fn leave(&self) -> actor::HolochainP2pResult<()> {
        self.sender
            .leave((*self.dna_hash).clone(), (*self.from_agent).clone())
            .await
    }

    /// Invoke a zome function on a remote node (if you have been granted the capability).
    async fn call_remote(
        &self,
        to_agent: AgentPubKey,
        zome_name: ZomeName,
        fn_name: FunctionName,
        cap: Option<CapSecret>,
        payload: ExternIO,
    ) -> actor::HolochainP2pResult<SerializedBytes> {
        self.sender
            .call_remote(
                (*self.dna_hash).clone(),
                (*self.from_agent).clone(),
                to_agent,
                zome_name,
                fn_name,
                cap,
                payload,
            )
            .await
    }

    /// Publish data to the correct neighborhood.
    async fn publish(
        &self,
        request_validation_receipt: bool,
        countersigning_session: bool,
        dht_hash: holo_hash::AnyDhtHash,
        ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
        timeout_ms: Option<u64>,
    ) -> actor::HolochainP2pResult<()> {
        self.sender
            .publish(
                (*self.dna_hash).clone(),
                (*self.from_agent).clone(),
                request_validation_receipt,
                countersigning_session,
                dht_hash,
                ops,
                timeout_ms,
            )
            .await
    }

    /// Request a validation package.
    async fn get_validation_package(
        &self,
        request_from: AgentPubKey,
        header_hash: HeaderHash,
    ) -> actor::HolochainP2pResult<ValidationPackageResponse> {
        self.sender
            .get_validation_package(actor::GetValidationPackage {
                dna_hash: (*self.dna_hash).clone(),
                agent_pub_key: (*self.from_agent).clone(),
                request_from,
                header_hash,
            })
            .await
    }

    /// Get an entry from the DHT.
    async fn get(
        &self,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetOptions,
    ) -> actor::HolochainP2pResult<Vec<WireOps>> {
        self.sender
            .get(
                (*self.dna_hash).clone(),
                (*self.from_agent).clone(),
                dht_hash,
                options,
            )
            .instrument(tracing::debug_span!("HolochainP2p::get"))
            .await
    }

    /// Get metadata from the DHT.
    async fn get_meta(
        &self,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetMetaOptions,
    ) -> actor::HolochainP2pResult<Vec<MetadataSet>> {
        self.sender
            .get_meta(
                (*self.dna_hash).clone(),
                (*self.from_agent).clone(),
                dht_hash,
                options,
            )
            .await
    }

    /// Get links from the DHT.
    async fn get_links(
        &self,
        link_key: WireLinkKey,
        options: actor::GetLinksOptions,
    ) -> actor::HolochainP2pResult<Vec<WireLinkOps>> {
        self.sender
            .get_links(
                (*self.dna_hash).clone(),
                (*self.from_agent).clone(),
                link_key,
                options,
            )
            .await
    }

    /// Get agent activity from the DHT.
    async fn get_agent_activity(
        &self,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: actor::GetActivityOptions,
    ) -> actor::HolochainP2pResult<Vec<AgentActivityResponse<HeaderHash>>> {
        self.sender
            .get_agent_activity(
                (*self.dna_hash).clone(),
                (*self.from_agent).clone(),
                agent,
                query,
                options,
            )
            .await
    }

    /// Send a validation receipt to a remote node.
    async fn send_validation_receipt(
        &self,
        to_agent: AgentPubKey,
        receipt: SerializedBytes,
    ) -> actor::HolochainP2pResult<()> {
        self.sender
            .send_validation_receipt(
                (*self.dna_hash).clone(),
                to_agent,
                (*self.from_agent).clone(),
                receipt,
            )
            .await
    }

    /// Check if an agent is an authority for a hash.
    async fn authority_for_hash(
        &self,
        dht_hash: holo_hash::AnyDhtHash,
    ) -> actor::HolochainP2pResult<bool> {
        self.sender
            .authority_for_hash(
                (*self.dna_hash).clone(),
                (*self.from_agent).clone(),
                dht_hash,
            )
            .await
    }

    async fn countersigning_authority_response(
        &self,
        agents: Vec<AgentPubKey>,
        response: Vec<SignedHeader>,
    ) -> actor::HolochainP2pResult<()> {
        self.sender
            .countersigning_authority_response(
                (*self.dna_hash).clone(),
                (*self.from_agent).clone(),
                agents,
                response,
            )
            .await
    }

    async fn new_integrated_data(&self, authored: bool) -> actor::HolochainP2pResult<()> {
        self.sender
            .new_integrated_data((*self.dna_hash).clone(), authored)
            .await
    }
}

pub use kitsune_p2p::dht_arc;

mod test;
