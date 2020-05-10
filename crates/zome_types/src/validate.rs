use holo_hash_core::EntryHash;
use holochain_serialized_bytes::prelude::*;
use crate::zome_io::GuestOutput;
use crate::zome::ZomeName;

#[derive(Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidateCallbackResult {
    Valid,
    Invalid(String),
    /// subconscious needs to map this to either pending or abandoned based on context that the
    /// wasm can't possibly have
    UnresolvedDependencies(Vec<EntryHash>),
}

impl From<GuestOutput> for ValidateCallbackResult {
    fn from(callback_guest_output: GuestOutput) -> Self {
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
    Fail(ZomeName, String),
    UnresolvedDependencies(ZomeName, Vec<EntryHash>),
}

impl From<GuestOutput> for ValidationPackageCallbackResult {
    fn from(callback_guest_output: GuestOutput) -> Self {
        match callback_guest_output.try_into() {
            Ok(v) => v,
            Err(e) => ValidationPackageCallbackResult::Fail(ZomeName::unknown(), format!("{:?}", e)),
        }
    }
}
