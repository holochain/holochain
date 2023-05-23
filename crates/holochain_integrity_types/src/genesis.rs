//! Types related to the genesis process whereby a user commits their initial
//! records and validates them to the best of their ability. Full validation
//! may not be possible if network access is required, so they perform a
//! "self-check" (as in "check yourself before you wreck yourself") before
//! joining to ensure that they can catch any problems they can before being
//! subject to the scrutiny of their peers and facing possible rejection.

use crate::DnaInfoV1;
use holo_hash::AgentPubKey;
use holochain_serialized_bytes::prelude::*;

/// App-specific payload for proving membership in the membrane of the app
pub type MembraneProof = std::sync::Arc<SerializedBytes>;

/// Data passed into the genesis_self_check callback for verifying the initial
/// chain entries
#[derive(Debug, Serialize, Deserialize, SerializedBytes)]
pub struct GenesisSelfCheckDataV1 {
    /// The Dna action (1st record)
    pub dna_info: DnaInfoV1,

    /// The proof of membership provided by the AgentValidationPkg (2nd record)
    pub membrane_proof: Option<MembraneProof>,

    /// The 3rd record of the chain, the agent key
    pub agent_key: AgentPubKey,
}

/// Data passed into the genesis_self_check callback for verifying the initial
/// chain entries
/// The proof of membership provided by the AgentValidationPkg is the 2nd record
/// in the chain, but is provided as an argument to the callback for convenience.
#[derive(Debug, Serialize, Deserialize, SerializedBytes)]
pub struct GenesisSelfCheckDataV2(pub Option<MembraneProof>);

impl GenesisSelfCheckDataV2 {
    /// Accessor to inner membrane proof by ref.
    pub fn maybe_membrane_proof(&self) -> Option<&MembraneProof> {
        self.0.as_ref()
    }
}

/// Alias to the current version of `GenesisSelfCheckData`.
pub type GenesisSelfCheckData = GenesisSelfCheckDataV2;
