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
/// chain entries. DnaInfo can be read with a call to `dna_info` within the
/// self check callback, it is elided here to minimise/stabilise the callback
/// function signature.
#[derive(Debug, Serialize, Deserialize, SerializedBytes)]
pub struct GenesisSelfCheckDataV2 {
    /// The proof of membership that will be the AgentValidationPkg (2nd record).
    pub membrane_proof: Option<MembraneProof>,
    /// Will be the 3rd record of the chain, the agent key.
    pub agent_key: AgentPubKey,
}

/// Alias to the current version of `GenesisSelfCheckData`.
pub type GenesisSelfCheckData = GenesisSelfCheckDataV2;
