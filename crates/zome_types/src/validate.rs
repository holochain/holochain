use holo_hash_core::EntryHash;
use holochain_serialized_bytes::prelude::*;

#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidateCallbackResult {
    Valid,
    Invalid(String),
    /// subconscious needs to map this to either pending or abandoned based on context that the
    /// wasm can't possibly have
    UnresolvedDependencies(Vec<EntryHash>),
}

#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub struct ValidationPackage;

#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidationPackageCallbackResult {
    Success(ValidationPackage),
    Fail(String),
}
