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
    Visibility(pub),
    Name(HolochainP2pEvent),
    Error(super::HolochainP2pError),
    Api {
        CallRemote(
            "A remote node is attempting to make a remote call on us.",
            CallRemoteEvt,
            (), // TODO - proper return type
        ),
        Publish(
            "A remote node is publishing data in a range we claim to be holding.",
            PublishEvt,
            (), // TODO - proper return type
        ),
        GetValidationPackage(
            "A remote node is requesting a validation package.",
            GetValidationPackageEvt,
            (), // TODO - proper return type
        ),
        Get(
            "A remote node is requesting entry data from us.",
            GetEvt,
            (), // TODO - proper return type
        ),
        GetLinks(
            "A remote node is requesting link data from us.",
            GetLinksEvt,
            (), // TODO - proper return type
        ),
        ListDhtOpHashes(
            "The p2p module wishes to query our DhtOpHash store.",
            ListDhtOpHashesEvt,
            (), // TODO - proper return type
        ),
        FetchDhtOps(
            "The p2p module needs access to the content for a given set of DhtOpHashes.",
            FetchDhtOpsEvt,
            (), // TODO - proper return type
        ),
        SignNetworkData(
            "P2p operations require cryptographic signatures and validation.",
            SignNetworkDataEvt,
            Signature,
        ),
    }
}

/// Receiver type for incoming holochain p2p events.
pub type HolochainP2pEventReceiver = futures::channel::mpsc::Receiver<HolochainP2pEvent>;
