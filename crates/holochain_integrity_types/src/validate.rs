use holo_hash::AnyDhtHash;
use holochain_serialized_bytes::prelude::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidateCallbackResult {
    Valid,
    Invalid(String),
    /// Subconscious needs to map this to either pending or abandoned based on context that the
    /// wasm can't possibly have.
    UnresolvedDependencies(Vec<AnyDhtHash>),
}

/// The level of validation package required by
/// an entry.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum RequiredValidationType {
    /// Just the element (default)
    Element,
    /// All chain items of the same entry type
    SubChain,
    /// The entire chain
    Full,
    /// A custom package set by the zome
    Custom,
}

impl Default for RequiredValidationType {
    fn default() -> Self {
        Self::Element
    }
}
