use crate::chain::ChainFilter;
use holo_hash::AgentPubKey;
use holo_hash::AnyDhtHash;
use holochain_serialized_bytes::prelude::*;
use ts_rs::TS;
use export_types_config::EXPORT_TS_TYPES_FILE;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes, TS)]
#[ts(export, export_to = EXPORT_TS_TYPES_FILE)]
pub enum ValidateCallbackResult {
    Valid,
    Invalid(String),
    /// Subconscious needs to map this to either pending or abandoned based on context that the
    /// wasm can't possibly have.
    UnresolvedDependencies(UnresolvedDependencies),
}

/// Unresolved dependencies that are either a set of hashes
/// or an agent activity query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT_TS_TYPES_FILE)]
pub enum UnresolvedDependencies {
    Hashes(Vec<AnyDhtHash>),
    AgentActivity(AgentPubKey, ChainFilter),
}
