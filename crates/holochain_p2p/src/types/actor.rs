//! Module containing the HolochainP2p actor definition.

use crate::*;

/// Request a validation package.
pub struct GetValidationPackage {
    /// The dna_hash / space_hash context.
    pub dna_hash: DnaHash,
    /// The agent_id / agent_pub_key context.
    pub agent_pub_key: AgentPubKey,
    // TODO - parameters
}

/// Get an entry from the DHT.
pub struct Get {
    /// The dna_hash / space_hash context.
    pub dna_hash: DnaHash,
    /// The agent_id / agent_pub_key context.
    pub agent_pub_key: AgentPubKey,
    // TODO - parameters
}

/// Get links from the DHT.
pub struct GetLinks {
    /// The dna_hash / space_hash context.
    pub dna_hash: DnaHash,
    /// The agent_id / agent_pub_key context.
    pub agent_pub_key: AgentPubKey,
    // TODO - parameters
}

ghost_actor::ghost_actor! {
    /// The HolochainP2pSender struct allows controlling the HolochainP2p
    /// actor instance.
    pub actor HolochainP2p<HolochainP2pError> {
        /// The p2p module must be informed at runtime which dna/agent pairs it should be tracking.
        fn join(dna_hash: DnaHash, agent_pub_key: AgentPubKey) -> ();

        /// If a cell is deactivated, we'll need to \"leave\" the network module as well.
        fn leave(dna_hash: DnaHash, agent_pub_key: AgentPubKey) -> ();

        /// Invoke a zome function on a remote node (if you have been granted the capability).
        fn call_remote(dna_hash: DnaHash, agent_pub_key: AgentPubKey, request: SerializedBytes) -> SerializedBytes;

        /// Publish data to the correct neigborhood.
        fn publish(
            dna_hash: DnaHash,
            from_agent: AgentPubKey,
            request_validation_receipt: bool,
            entry_hash: holochain_types::composite_hash::AnyDhtHash,
            ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
            timeout_ms: u64,
        ) -> ();

        /// Request a validation package.
        fn get_validation_package(input: GetValidationPackage) -> (); // TODO - proper return type

        /// Get an entry from the DHT.
        fn get(input: Get) -> (); // TODO - proper return type

        /// Get links from the DHT.
        fn get_links(input: GetLinks) -> (); // TODO - proper return type

        /// Send a validation receipt to a remote node.
        fn send_validation_receipt(dna_hash: DnaHash, agent_pub_key: AgentPubKey, receipt: SerializedBytes) -> ();
    }
}

impl HolochainP2pSender {
    /// Partially apply dna_hash && agent_pub_key to this sender,
    /// binding it to a specific cell context.
    pub fn into_cell(self, dna_hash: DnaHash, from_agent: AgentPubKey) -> crate::HolochainP2pCell {
        crate::HolochainP2pCell {
            sender: self,
            dna_hash: Arc::new(dna_hash),
            from_agent: Arc::new(from_agent),
        }
    }

    /// Clone and partially apply dna_hash && agent_pub_key to this sender,
    /// binding it to a specific cell context.
    pub fn to_cell(&self, dna_hash: DnaHash, from_agent: AgentPubKey) -> crate::HolochainP2pCell {
        self.clone().into_cell(dna_hash, from_agent)
    }
}
