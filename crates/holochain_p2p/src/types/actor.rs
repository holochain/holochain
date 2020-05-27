//! Module containing the HolochainP2p actor definition.

use crate::*;

/// Invoke a zome function on a remote node (if you have been granted the capability).
pub struct CallRemote {
    /// The dna_hash / space_hash context.
    pub dna_hash: DnaHash,
    /// The agent_id / agent_pub_key context.
    pub agent_pub_key: AgentPubKey,
    // TODO - parameters
}

/// Publish data to the correct neigborhood.
pub struct Publish {
    /// The dna_hash / space_hash context.
    pub dna_hash: DnaHash,
    /// The agent_id / agent_pub_key context.
    pub agent_pub_key: AgentPubKey,
    // TODO - parameters
}

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
        fn call_remote(input: CallRemote) -> (); // TODO - proper return type
        /// Publish data to the correct neigborhood.
        fn publish(input: Publish) -> (); // TODO - proper return type
        /// Request a validation package.
        fn get_validation_package(input: GetValidationPackage) -> (); // TODO - proper return type
        /// Get an entry from the DHT.
        fn get(input: Get) -> (); // TODO - proper return type
        /// Get links from the DHT.
        fn get_links(input: GetLinks) -> (); // TODO - proper return type
    }
}

impl HolochainP2pSender {
    /// Partially apply dna_hash && agent_pub_key to this sender,
    /// binding it to a specific cell context.
    pub fn into_cell(
        self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
    ) -> crate::HolochainP2pCell {
        crate::HolochainP2pCell {
            sender: self,
            dna_hash: Arc::new(dna_hash),
            agent_pub_key: Arc::new(agent_pub_key),
        }
    }

    /// Clone and partially apply dna_hash && agent_pub_key to this sender,
    /// binding it to a specific cell context.
    pub fn to_cell(
        &self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
    ) -> crate::HolochainP2pCell {
        self.clone().into_cell(dna_hash, agent_pub_key)
    }
}
