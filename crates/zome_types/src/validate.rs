use holo_hash_core::EntryHash;
use holochain_serialized_bytes::prelude::*;
use crate::zome_io::CallbackGuestOutput;

#[derive(Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidateCallbackResult {
    Valid,
    Invalid(String),
    /// subconscious needs to map this to either pending or abandoned based on context that the
    /// wasm can't possibly have
    UnresolvedDependencies(Vec<EntryHash>),
}

impl From<CallbackGuestOutput> for ValidateCallbackResult {
    fn from(callback_guest_output: CallbackGuestOutput) -> Self {
        match callback_guest_output.try_into() {
            Ok(v) => v,
            Err(e) => Self::Invalid(format!("{:?}", e)),
        }
    }
}

#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub struct ValidationPackage;

#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidationPackageCallbackResult {
    Success(ValidationPackage),
    Fail(String),
}
