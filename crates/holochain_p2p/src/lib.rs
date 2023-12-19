#![deny(missing_docs)]
//! holochain specific wrapper around more generic p2p module

use holo_hash::*;
use holochain_serialized_bytes::prelude::*;
use holochain_types::prelude::*;
use kitsune_p2p::dependencies::kitsune_p2p_fetch::OpHashSized;
use std::sync::Arc;

mod types;
pub use types::actor::FetchContextExt;
pub use types::actor::HolochainP2pRef;
pub use types::actor::HolochainP2pSender;
pub use types::AgentPubKeyExt; // why is this not included by * above???
pub use types::*;

mod spawn;
use ghost_actor::dependencies::tracing;
use ghost_actor::dependencies::tracing_futures::Instrument;
use kitsune_p2p_types::agent_info::AgentInfoSigned;
pub use spawn::*;
#[cfg(feature = "test_utils")]
pub use test::stub_network;
#[cfg(feature = "test_utils")]
pub use test::HolochainP2pDnaFixturator;

pub use kitsune_p2p;

#[allow(clippy::too_many_arguments)]
#[async_trait::async_trait]
/// A wrapper around HolochainP2pSender that partially applies the dna_hash / agent_pub_key.
/// I.e. a sender that is tied to a specific cell.
pub trait HolochainP2pDnaT: Send + Sync {
    /// owned getter
    fn dna_hash(&self) -> DnaHash;

    /// The p2p module must be informed at runtime which dna/agent pairs it should be tracking.
    async fn join(
        &self,
        agent: AgentPubKey,
        maybe_agent_info: Option<AgentInfoSigned>,
        initial_arc: Option<crate::dht_arc::DhtArc>,
    ) -> actor::HolochainP2pResult<()>;

    /// If a cell is disabled, we'll need to \"leave\" the network module as well.
    async fn leave(&self, agent: AgentPubKey) -> actor::HolochainP2pResult<()>;

    /// Invoke a zome function on a remote node (if you have been granted the capability).
    #[allow(clippy::too_many_arguments)]
    async fn call_remote(
        &self,
        from_agent: AgentPubKey,
        from_signature: Signature,
        to_agent: AgentPubKey,
        zome_name: ZomeName,
        fn_name: FunctionName,
        cap_secret: Option<CapSecret>,
        payload: ExternIO,
        nonce: Nonce256Bits,
        expires_at: Timestamp,
    ) -> actor::HolochainP2pResult<SerializedBytes>;

    /// Invoke a zome function on a remote node (if you have been granted the capability).
    /// This is a fire-and-forget operation, a best effort will be made
    /// to forward the signal, but if the conductor network is overworked
    /// it may decide not to deliver some of the signals.
    async fn send_remote_signal(
        &self,
        from_agent: AgentPubKey,
        to_agent_list: Vec<(Signature, AgentPubKey)>,
        zome_name: ZomeName,
        fn_name: FunctionName,
        cap: Option<CapSecret>,
        payload: ExternIO,
        nonce: Nonce256Bits,
        expires_at: Timestamp,
    ) -> actor::HolochainP2pResult<()>;

    /// Publish data to the correct neighborhood.
    #[allow(clippy::ptr_arg)]
    async fn publish(
        &self,
        request_validation_receipt: bool,
        countersigning_session: bool,
        basis_hash: holo_hash::OpBasis,
        source: AgentPubKey,
        op_hash_list: Vec<OpHashSized>,
        timeout_ms: Option<u64>,
        reflect_ops: Option<Vec<DhtOp>>,
    ) -> actor::HolochainP2pResult<()>;

    /// Publish a countersigning op.
    async fn publish_countersign(
        &self,
        flag: bool,
        basis_hash: holo_hash::OpBasis,
        op: DhtOp,
    ) -> actor::HolochainP2pResult<()>;

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

    /// Get a count of links from the DHT.
    async fn count_links(
        &self,
        query: WireLinkQuery,
    ) -> actor::HolochainP2pResult<CountLinksResponse>;

    /// Get agent activity from the DHT.
    async fn get_agent_activity(
        &self,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: actor::GetActivityOptions,
    ) -> actor::HolochainP2pResult<Vec<AgentActivityResponse<ActionHash>>>;

    /// Get agent deterministic activity from the DHT.
    async fn must_get_agent_activity(
        &self,
        author: AgentPubKey,
        filter: holochain_zome_types::chain::ChainFilter,
    ) -> actor::HolochainP2pResult<Vec<MustGetAgentActivityResponse>>;

    /// Send a validation receipt to a remote node.
    async fn send_validation_receipts(
        &self,
        to_agent: AgentPubKey,
        receipts: ValidationReceiptBundle,
    ) -> actor::HolochainP2pResult<()>;

    /// Check if an agent is an authority for a hash.
    async fn authority_for_hash(
        &self,
        basis: holo_hash::OpBasis,
    ) -> actor::HolochainP2pResult<bool>;

    /// Messages between agents driving a countersigning session.
    async fn countersigning_session_negotiation(
        &self,
        agents: Vec<AgentPubKey>,
        message: event::CountersigningSessionNegotiationMessage,
    ) -> actor::HolochainP2pResult<()>;

    /// New data has been integrated and is ready for gossiping.
    async fn new_integrated_data(&self) -> actor::HolochainP2pResult<()>;

    /// Access to the specified CHC
    fn chc(&self) -> Option<ChcImpl>;
}

// TODO Refactor so that HolochainP2pDna is passed as an Arc rather than being Clone so that this can
//      go back to being derived rather than hand written.
#[cfg(feature = "test_utils")]
mockall::mock! {
    pub HolochainP2pDnaT {}
    #[async_trait::async_trait]
    impl HolochainP2pDnaT for HolochainP2pDnaT {
        fn dna_hash(&self) -> DnaHash;
        async fn join(
            &self,
            agent: AgentPubKey,
            maybe_agent_info: Option<AgentInfoSigned>,
            initial_arc: Option<crate::dht_arc::DhtArc>,
        ) -> actor::HolochainP2pResult<()>;
        async fn leave(&self, agent: AgentPubKey) -> actor::HolochainP2pResult<()>;
        #[allow(clippy::too_many_arguments)]
        async fn call_remote(
            &self,
            from_agent: AgentPubKey,
            from_signature: Signature,
            to_agent: AgentPubKey,
            zome_name: ZomeName,
            fn_name: FunctionName,
            cap_secret: Option<CapSecret>,
            payload: ExternIO,
            nonce: Nonce256Bits,
            expires_at: Timestamp,
        ) -> actor::HolochainP2pResult<SerializedBytes>;
        #[allow(clippy::too_many_arguments)]
        async fn send_remote_signal(
            &self,
            from_agent: AgentPubKey,
            to_agent_list: Vec<(Signature, AgentPubKey)>,
            zome_name: ZomeName,
            fn_name: FunctionName,
            cap: Option<CapSecret>,
            payload: ExternIO,
            nonce: Nonce256Bits,
            expires_at: Timestamp,
        ) -> actor::HolochainP2pResult<()>;
        #[allow(clippy::too_many_arguments)]
        async fn publish(
            &self,
            request_validation_receipt: bool,
            countersigning_session: bool,
            basis_hash: holo_hash::OpBasis,
            source: AgentPubKey,
            op_hash_list: Vec<OpHashSized>,
            timeout_ms: Option<u64>,
            reflect_ops: Option<Vec<DhtOp>>,
        ) -> actor::HolochainP2pResult<()>;
        async fn publish_countersign(
            &self,
            flag: bool,
            basis_hash: holo_hash::OpBasis,
            op: DhtOp,
        ) -> actor::HolochainP2pResult<()>;
        async fn get(
            &self,
            dht_hash: holo_hash::AnyDhtHash,
            options: actor::GetOptions,
        ) -> actor::HolochainP2pResult<Vec<WireOps>>;
        async fn get_meta(
            &self,
            dht_hash: holo_hash::AnyDhtHash,
            options: actor::GetMetaOptions,
        ) -> actor::HolochainP2pResult<Vec<MetadataSet>>;
        async fn get_links(
            &self,
            link_key: WireLinkKey,
            options: actor::GetLinksOptions,
        ) -> actor::HolochainP2pResult<Vec<WireLinkOps>>;
        async fn count_links(
            &self,
            query: WireLinkQuery,
        ) -> actor::HolochainP2pResult<CountLinksResponse>;
        async fn get_agent_activity(
            &self,
            agent: AgentPubKey,
            query: ChainQueryFilter,
            options: actor::GetActivityOptions,
        ) -> actor::HolochainP2pResult<Vec<AgentActivityResponse<ActionHash>>>;
        async fn must_get_agent_activity(
            &self,
            author: AgentPubKey,
            filter: holochain_zome_types::chain::ChainFilter,
        ) -> actor::HolochainP2pResult<Vec<MustGetAgentActivityResponse>>;
        async fn send_validation_receipts(
            &self,
            to_agent: AgentPubKey,
            receipts: ValidationReceiptBundle,
        ) -> actor::HolochainP2pResult<()>;
        async fn authority_for_hash(
            &self,
            basis: holo_hash::OpBasis,
        ) -> actor::HolochainP2pResult<bool>;
        async fn countersigning_session_negotiation(
            &self,
            agents: Vec<AgentPubKey>,
            message: event::CountersigningSessionNegotiationMessage,
        ) -> actor::HolochainP2pResult<()>;
        async fn new_integrated_data(&self) -> actor::HolochainP2pResult<()>;
        fn chc(&self) -> Option<ChcImpl>;
    }
    impl Clone for HolochainP2pDnaT {
        fn clone(&self) -> Self;
    }
}

/// A wrapper around HolochainP2pSender that partially applies the dna_hash / agent_pub_key.
/// I.e. a sender that is tied to a specific cell.
#[derive(Clone)]
pub struct HolochainP2pDna {
    sender: ghost_actor::GhostSender<actor::HolochainP2p>,
    dna_hash: Arc<DnaHash>,
    chc: Option<ChcImpl>,
}

/// A CHC implementation
pub type ChcImpl = Arc<dyn 'static + Send + Sync + ChainHeadCoordinatorExt>;

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
        initial_arc: Option<crate::dht_arc::DhtArc>,
    ) -> actor::HolochainP2pResult<()> {
        self.sender
            .join(
                (*self.dna_hash).clone(),
                agent,
                maybe_agent_info,
                initial_arc,
            )
            .await
    }

    /// If a cell is disabled, we'll need to \"leave\" the network module as well.
    async fn leave(&self, agent: AgentPubKey) -> actor::HolochainP2pResult<()> {
        self.sender.leave((*self.dna_hash).clone(), agent).await
    }

    /// Invoke a zome function on a remote node (if you have been granted the capability).
    async fn call_remote(
        &self,
        from_agent: AgentPubKey,
        from_signature: Signature,
        to_agent: AgentPubKey,
        zome_name: ZomeName,
        fn_name: FunctionName,
        cap_secret: Option<CapSecret>,
        payload: ExternIO,
        nonce: Nonce256Bits,
        expires_at: Timestamp,
    ) -> actor::HolochainP2pResult<SerializedBytes> {
        self.sender
            .call_remote(
                (*self.dna_hash).clone(),
                from_agent,
                from_signature,
                to_agent,
                zome_name,
                fn_name,
                cap_secret,
                payload,
                nonce,
                expires_at,
            )
            .await
    }

    /// Invoke a zome function on a remote node (if you have been granted the capability).
    /// This is a fire-and-forget operation, a best effort will be made
    /// to forward the signal, but if the conductor network is overworked
    /// it may decide not to deliver some of the signals.
    async fn send_remote_signal(
        &self,
        from_agent: AgentPubKey,
        to_agent_list: Vec<(Signature, AgentPubKey)>,
        zome_name: ZomeName,
        fn_name: FunctionName,
        cap: Option<CapSecret>,
        payload: ExternIO,
        nonce: Nonce256Bits,
        expires_at: Timestamp,
    ) -> actor::HolochainP2pResult<()> {
        self.sender
            .send_remote_signal(
                (*self.dna_hash).clone(),
                from_agent,
                to_agent_list,
                zome_name,
                fn_name,
                cap,
                payload,
                nonce,
                expires_at,
            )
            .await
    }

    /// Publish data to the correct neighborhood.
    async fn publish(
        &self,
        request_validation_receipt: bool,
        countersigning_session: bool,
        basis_hash: holo_hash::OpBasis,
        source: AgentPubKey,
        op_hash_list: Vec<OpHashSized>,
        timeout_ms: Option<u64>,
        reflect_ops: Option<Vec<DhtOp>>,
    ) -> actor::HolochainP2pResult<()> {
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
    ) -> actor::HolochainP2pResult<()> {
        self.sender
            .publish_countersign((*self.dna_hash).clone(), flag, basis_hash, op)
            .await
    }

    /// Get [`DhtOp::StoreRecord`] or [`DhtOp::StoreEntry`] from the DHT.
    async fn get(
        &self,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetOptions,
    ) -> actor::HolochainP2pResult<Vec<WireOps>> {
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
    ) -> actor::HolochainP2pResult<Vec<MetadataSet>> {
        self.sender
            .get_meta((*self.dna_hash).clone(), dht_hash, options)
            .await
    }

    /// Get links from the DHT.
    async fn get_links(
        &self,
        link_key: WireLinkKey,
        options: actor::GetLinksOptions,
    ) -> actor::HolochainP2pResult<Vec<WireLinkOps>> {
        self.sender
            .get_links((*self.dna_hash).clone(), link_key, options)
            .await
    }

    /// Get a count of links from the DHT.
    async fn count_links(
        &self,
        query: WireLinkQuery,
    ) -> actor::HolochainP2pResult<CountLinksResponse> {
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
    ) -> actor::HolochainP2pResult<Vec<AgentActivityResponse<ActionHash>>> {
        self.sender
            .get_agent_activity((*self.dna_hash).clone(), agent, query, options)
            .await
    }

    async fn must_get_agent_activity(
        &self,
        author: AgentPubKey,
        filter: holochain_zome_types::chain::ChainFilter,
    ) -> actor::HolochainP2pResult<Vec<MustGetAgentActivityResponse>> {
        self.sender
            .must_get_agent_activity((*self.dna_hash).clone(), author, filter)
            .await
    }

    /// Send a validation receipt to a remote node.
    async fn send_validation_receipts(
        &self,
        to_agent: AgentPubKey,
        receipts: ValidationReceiptBundle,
    ) -> actor::HolochainP2pResult<()> {
        self.sender
            .send_validation_receipts((*self.dna_hash).clone(), to_agent, receipts)
            .await
    }

    /// Check if an agent is an authority for a hash.
    async fn authority_for_hash(
        &self,
        dht_hash: holo_hash::OpBasis,
    ) -> actor::HolochainP2pResult<bool> {
        self.sender
            .authority_for_hash((*self.dna_hash).clone(), dht_hash)
            .await
    }

    async fn countersigning_session_negotiation(
        &self,
        agents: Vec<AgentPubKey>,
        message: event::CountersigningSessionNegotiationMessage,
    ) -> actor::HolochainP2pResult<()> {
        self.sender
            .countersigning_session_negotiation((*self.dna_hash).clone(), agents, message)
            .await
    }

    async fn new_integrated_data(&self) -> actor::HolochainP2pResult<()> {
        self.sender
            .new_integrated_data((*self.dna_hash).clone())
            .await
    }

    fn chc(&self) -> Option<ChcImpl> {
        self.chc.clone()
    }
}

pub use kitsune_p2p::dht;
pub use kitsune_p2p::dht_arc;

#[allow(unused)]
#[cfg(any(test, feature = "test_utils"))]
mod test;
