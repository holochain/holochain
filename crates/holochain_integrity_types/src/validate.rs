use holo_hash::AgentPubKey;
use holo_hash::AnyDhtHash;
use holochain_serialized_bytes::prelude::*;

use crate::chain::ChainFilter;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidateCallbackResult {
    Valid,
    Invalid(String),
    /// Subconscious needs to map this to either pending or abandoned based on context that the
    /// wasm can't possibly have.
    UnresolvedDependencies(UnresolvedDependencies),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Unresolved dependencies that are either a set of hashes
/// or an agent activity query.
pub enum UnresolvedDependencies {
    Hashes(Vec<AnyDhtHash>),
    Activity(AgentPubKey, ChainFilter),
}

/// The level of validation package required by
/// an entry.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum RequiredValidationType {
    /// Just the record (default)
    Record,
    /// All chain items of the same entry type
    SubChain,
    /// The entire chain
    Full,
    /// A custom package set by the zome
    Custom,
}

impl Default for RequiredValidationType {
    fn default() -> Self {
        Self::Record
    }
}
