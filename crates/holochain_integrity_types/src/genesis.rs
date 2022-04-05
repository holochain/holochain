//! Types related to the genesis process whereby a user commits their initial
//! elements and validates them to the best of their ability. Full validation
//! may not be possible if network access is required, so they perform a
//! "self-check" (as in "check yourself before you wreck yourself") before
//! joining to ensure that they can catch any problems they can before being
//! subject to the scrutiny of their peers and facing possible rejection.

use holochain_serialized_bytes::prelude::*;

/// App-specific payload for proving membership in the membrane of the app
pub type MembraneProof = std::sync::Arc<SerializedBytes>;
