//! Module containing incoming events from the HolochainP2p actor.

use crate::*;

/// Get options help control how the get is processed at various levels.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GetOptions {
    /// Whether the remote-end should follow redirects or just return the
    /// requested entry.
    pub follow_redirects: bool,
}

impl From<&actor::GetOptions> for GetOptions {
    fn from(a: &actor::GetOptions) -> Self {
        Self {
            follow_redirects: a.follow_redirects,
        }
    }
}

ghost_actor::ghost_actor! {
    /// The HolochainP2pEvent stream allows handling events generated from
    /// the HolochainP2p actor.
    pub actor HolochainP2pEvent<super::HolochainP2pError> {
        /// A remote node is attempting to make a remote call on us.
        fn call_remote(
            dna_hash: DnaHash,
            to_agent: AgentPubKey,
            zome_name: ZomeName,
            fn_name: String,
            cap: CapSecret,
            request: SerializedBytes,
        ) -> SerializedBytes;

        /// A remote node is publishing data in a range we claim to be holding.
        fn publish(
            dna_hash: DnaHash,
            to_agent: AgentPubKey,
            from_agent: AgentPubKey,
            request_validation_receipt: bool,
            dht_hash: holochain_types::composite_hash::AnyDhtHash,
            ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
        ) -> ();

        /// A remote node is requesting a validation package.
        fn get_validation_package(
            // The dna_hash / space_hash context.
            dna_hash: DnaHash,
            // The agent_id / agent_pub_key context.
            to_agent: AgentPubKey,
            // TODO - parameters
        ) -> (); // TODO - proper return type

        /// A remote node is requesting entry data from us.
        fn get(
            dna_hash: DnaHash,
            to_agent: AgentPubKey,
            dht_hash: holochain_types::composite_hash::AnyDhtHash,
            options: GetOptions,
        ) -> SerializedBytes;

        /// A remote node is requesting link data from us.
        fn get_links(
            // The dna_hash / space_hash context.
            dna_hash: DnaHash,
            // The agent_id / agent_pub_key context.
            to_agent: AgentPubKey,
            // TODO - parameters
        ) -> (); // TODO - proper return type

        /// A remote node has sent us a validation receipt.
        fn validation_receipt_received(
            dna_hash: DnaHash,
            to_agent: AgentPubKey,
            receipt: SerializedBytes,
        ) -> ();

        /// The p2p module wishes to query our DhtOpHash store.
        fn list_dht_op_hashes(
            // The dna_hash / space_hash context.
            dna_hash: DnaHash,
            // The agent_id / agent_pub_key context.
            to_agent: AgentPubKey,
            // TODO - parameters
        ) -> (); // TODO - proper return type

        /// The p2p module needs access to the content for a given set of DhtOpHashes.
        fn fetch_dht_ops(
            // The dna_hash / space_hash context.
            dna_hash: DnaHash,
            // The agent_id / agent_pub_key context.
            to_agent: AgentPubKey,
            // TODO - parameters
        ) -> (); // TODO - proper return type

        /// P2p operations require cryptographic signatures and validation.
        fn sign_network_data(
            // The dna_hash / space_hash context.
            dna_hash: DnaHash,
            // The agent_id / agent_pub_key context.
            to_agent: AgentPubKey,
            // The data to sign.
            data: Vec<u8>,
        ) -> Signature;
    }
}

/// utility macro to make it more ergonomic to access the enum variants
macro_rules! match_p2p_evt {
    ($h:ident => |$i:ident| { $($t:tt)* }) => {
        match $h {
            HolochainP2pEvent::CallRemote { $i, .. } => { $($t)* }
            HolochainP2pEvent::Publish { $i, .. } => { $($t)* }
            HolochainP2pEvent::GetValidationPackage { $i, .. } => { $($t)* }
            HolochainP2pEvent::Get { $i, .. } => { $($t)* }
            HolochainP2pEvent::GetLinks { $i, .. } => { $($t)* }
            HolochainP2pEvent::ValidationReceiptReceived { $i, .. } => { $($t)* }
            HolochainP2pEvent::ListDhtOpHashes { $i, .. } => { $($t)* }
            HolochainP2pEvent::FetchDhtOps { $i, .. } => { $($t)* }
            HolochainP2pEvent::SignNetworkData { $i, .. } => { $($t)* }
        }
    };
}

impl HolochainP2pEvent {
    /// The dna_hash associated with this network p2p event.
    pub fn dna_hash(&self) -> &DnaHash {
        match_p2p_evt!(self => |dna_hash| { dna_hash })
    }

    /// The agent_pub_key associated with this network p2p event.
    pub fn as_to_agent(&self) -> &AgentPubKey {
        match_p2p_evt!(self => |to_agent| { to_agent })
    }
}

/// Receiver type for incoming holochain p2p events.
pub type HolochainP2pEventReceiver = futures::channel::mpsc::Receiver<HolochainP2pEvent>;
