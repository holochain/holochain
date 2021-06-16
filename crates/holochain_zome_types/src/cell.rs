//! A "Cell" represents a DNA/AgentId pair - a space where one dna/agent
//! can track its source chain and service network requests / responses.

use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_serialized_bytes::prelude::*;
use std::fmt;

/// The unique identifier for a Cell.
/// Cells are uniquely determined by this pair - this pair is necessary
/// and sufficient to refer to a cell in a conductor
#[derive(
    Clone, Debug, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes,
)]
pub struct CellId(DnaHash, AgentPubKey);

impl fmt::Display for CellId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Cell({}, {})", self.dna_hash(), self.agent_pubkey())
    }
}

impl CellId {
    /// Create a CellId from its components
    pub fn new(dna_hash: DnaHash, agent_pubkey: AgentPubKey) -> Self {
        CellId(dna_hash, agent_pubkey)
    }

    /// The dna hash/address for this cell.
    pub fn dna_hash(&self) -> &DnaHash {
        &self.0
    }

    /// The agent id / public key for this cell.
    pub fn agent_pubkey(&self) -> &AgentPubKey {
        &self.1
    }

    /// Into [DnaHash] and [AgentPubKey]
    pub fn into_dna_and_agent(self) -> (DnaHash, AgentPubKey) {
        (self.0, self.1)
    }
}

impl From<(DnaHash, AgentPubKey)> for CellId {
    fn from(pair: (DnaHash, AgentPubKey)) -> Self {
        Self(pair.0, pair.1)
    }
}
