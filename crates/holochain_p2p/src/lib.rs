#![deny(missing_docs)]
//! holochain specific wrapper around more generic p2p module

use holo_hash::*;
use holochain_keystore::*;
use std::sync::Arc;

mod types;
pub use types::*;

mod spawn;
pub use spawn::*;

/// A wrapper around HolochainP2pSender that partially applies the dna_hash / agent_pub_key.
/// I.e. a sender that is tied to a specific cell.
#[derive(Clone)]
pub struct HolochainP2pCell {
    sender: actor::HolochainP2pSender,
    dna_hash: Arc<DnaHash>,
    agent_pub_key: Arc<AgentPubKey>,
}

impl HolochainP2pCell {
    /// The p2p module must be informed at runtime which dna/agent pairs it should be tracking.
    pub async fn join(&mut self) -> actor::HolochainP2pResult<()> {
        self.sender
            .join((*self.dna_hash).clone(), (*self.agent_pub_key).clone())
            .await
    }

    /// If a cell is deactivated, we'll need to \"leave\" the network module as well.
    pub async fn leave(&mut self) -> actor::HolochainP2pResult<()> {
        self.sender
            .leave((*self.dna_hash).clone(), (*self.agent_pub_key).clone())
            .await
    }

    /// Invoke a zome function on a remote node (if you have been granted the capability).
    pub async fn call_remote(&mut self) -> actor::HolochainP2pResult<()> {
        self.sender
            .call_remote(actor::CallRemote {
                dna_hash: (*self.dna_hash).clone(),
                agent_pub_key: (*self.agent_pub_key).clone(),
            })
            .await
    }

    /// Publish data to the correct neigborhood.
    pub async fn publish(&mut self) -> actor::HolochainP2pResult<()> {
        self.sender
            .publish(actor::Publish {
                dna_hash: (*self.dna_hash).clone(),
                agent_pub_key: (*self.agent_pub_key).clone(),
            })
            .await
    }

    /// Request a validation package.
    pub async fn get_validation_package(&mut self) -> actor::HolochainP2pResult<()> {
        self.sender
            .get_validation_package(actor::GetValidationPackage {
                dna_hash: (*self.dna_hash).clone(),
                agent_pub_key: (*self.agent_pub_key).clone(),
            })
            .await
    }

    /// Get an entry from the DHT.
    pub async fn get(&mut self) -> actor::HolochainP2pResult<()> {
        self.sender
            .get(actor::Get {
                dna_hash: (*self.dna_hash).clone(),
                agent_pub_key: (*self.agent_pub_key).clone(),
            })
            .await
    }

    /// Get links from the DHT.
    pub async fn get_links(&mut self) -> actor::HolochainP2pResult<()> {
        self.sender
            .get_links(actor::GetLinks {
                dna_hash: (*self.dna_hash).clone(),
                agent_pub_key: (*self.agent_pub_key).clone(),
            })
            .await
    }
}
