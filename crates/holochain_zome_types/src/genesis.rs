//! Types related to the genesis process whereby a user commits their initial
//! elements and validates them to the best of their ability. Full validation
//! may not be possible if network access is required, so they perform a
//! "self-check" (as in "check yourself before you wreck yourself") before
//! joining to ensure that they can catch any problems they can before being
//! subject to the scrutiny of their peers and facing possible rejection.

use crate::DnaDef;
use holo_hash::AgentPubKey;
use holochain_serialized_bytes::prelude::*;

/// App-specific payload for proving membership in the membrane of the app
pub type MembraneProof = std::sync::Arc<SerializedBytes>;

/// Data passed into the genesis_self_check callback for verifying the initial
/// chain entries
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes)]
pub struct GenesisSelfCheckData {
    /// The Dna header (1st element)
    pub dna_def: DnaDef,

    /// The proof of membership provided by the AgentValidationPkg (2nd element)
    pub membrane_proof: Option<MembraneProof>,

    /// The 3rd element of the chain, the agent key
    pub agent_key: AgentPubKey,
}
