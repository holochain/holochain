//! Module containing incoming events from the HolochainP2p actor.

use crate::*;

/// A remote node is attempting to make a remote call on us.
pub struct CallRemoteEvt {
    /// The dna_hash / space_hash context.
    pub dna_hash: DnaHash,
    /// The agent_id / agent_pub_key context.
    pub agent_pub_key: AgentPubKey,
    // TODO - parameters
}

/// A remote node is publishing data in a range we claim to be holding.
pub struct PublishEvt {
    /// The dna_hash / space_hash context.
    pub dna_hash: DnaHash,
    /// The agent_id / agent_pub_key context.
    pub agent_pub_key: AgentPubKey,
    // TODO - parameters
}

/// A remote node is requesting a validation package.
pub struct GetValidationPackageEvt {
    /// The dna_hash / space_hash context.
    pub dna_hash: DnaHash,
    /// The agent_id / agent_pub_key context.
    pub agent_pub_key: AgentPubKey,
    // TODO - parameters
}

/// A remote node is requesting entry data from us.
pub struct GetEvt {
    /// The dna_hash / space_hash context.
    pub dna_hash: DnaHash,
    /// The agent_id / agent_pub_key context.
    pub agent_pub_key: AgentPubKey,
    // TODO - parameters
}

/// A remote node is requesting link data from us.
pub struct GetLinksEvt {
    /// The dna_hash / space_hash context.
    pub dna_hash: DnaHash,
    /// The agent_id / agent_pub_key context.
    pub agent_pub_key: AgentPubKey,
    // TODO - parameters
}

/// The p2p module wishes to query our DhtOpHash store.
pub struct ListDhtOpHashesEvt {
    /// The dna_hash / space_hash context.
    pub dna_hash: DnaHash,
    /// The agent_id / agent_pub_key context.
    pub agent_pub_key: AgentPubKey,
    // TODO - parameters
}

/// The p2p module needs access to the content for a given set of DhtOpHashes.
pub struct FetchDhtOpsEvt {
    /// The dna_hash / space_hash context.
    pub dna_hash: DnaHash,
    /// The agent_id / agent_pub_key context.
    pub agent_pub_key: AgentPubKey,
    // TODO - parameters
}

/// P2p operations require cryptographic signatures and validation.
pub struct SignNetworkDataEvt {
    /// The dna_hash / space_hash context.
    pub dna_hash: DnaHash,
    /// The agent_id / agent_pub_key context.
    pub agent_pub_key: AgentPubKey,
    /// The data to sign.
    pub data: Vec<u8>,
}

ghost_actor::ghost_chan! {
    /// The HolochainP2pEvent stream allows handling events generated from
    /// the HolochainP2p actor.
    pub chan HolochainP2pEvent<super::HolochainP2pError> {
        /// A remote node is attempting to make a remote call on us.
        fn call_remote(input: CallRemoteEvt) -> (); // TODO - proper return type

        /// A remote node is publishing data in a range we claim to be holding.
        fn publish(input: PublishEvt) -> (); // TODO - proper return type

        /// A remote node is requesting a validation package.
        fn get_validation_package(input: GetValidationPackageEvt) -> (); // TODO - proper return type

        /// A remote node is requesting entry data from us.
        fn get(input: GetEvt) -> (); // TODO - proper return type

        /// A remote node is requesting link data from us.
        fn get_links(input: GetLinksEvt) -> (); // TODO - proper return type

        /// The p2p module wishes to query our DhtOpHash store.
        fn list_dht_op_hashes(input: ListDhtOpHashesEvt) -> (); // TODO - proper return type

        /// The p2p module needs access to the content for a given set of DhtOpHashes.
        fn fetch_dht_ops(input: FetchDhtOpsEvt) -> (); // TODO - proper return type

        /// P2p operations require cryptographic signatures and validation.
        fn sign_network_data(input: SignNetworkDataEvt) -> Signature;
    }
}

/// Receiver type for incoming holochain p2p events.
pub type HolochainP2pEventReceiver = futures::channel::mpsc::Receiver<HolochainP2pEvent>;
