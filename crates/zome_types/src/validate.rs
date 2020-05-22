use crate::zome_io::GuestOutput;
use crate::CallbackResult;
use holo_hash_core::EntryHash;
use holochain_serialized_bytes::prelude::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidateCallbackResult {
    Valid,
    Invalid(String),
    /// subconscious needs to map this to either pending or abandoned based on context that the
    /// wasm can't possibly have
    UnresolvedDependencies(Vec<EntryHash>),
}

impl CallbackResult for ValidateCallbackResult {
    fn is_definitive(&self) -> bool {
        match self {
            ValidateCallbackResult::Invalid(_) => true,
            _ => false,
        }
    }
}

impl From<GuestOutput> for ValidateCallbackResult {
    fn from(guest_output: GuestOutput) -> Self {
        match guest_output.into_inner().try_into() {
            Ok(v) => v,
            Err(e) => Self::Invalid(format!("{:?}", e)),
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub struct ValidationPackage;

#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidationPackageCallbackResult {
    Success(ValidationPackage),
    Fail(String),
    UnresolvedDependencies(Vec<EntryHash>),
}

impl From<GuestOutput> for ValidationPackageCallbackResult {
    fn from(guest_output: GuestOutput) -> Self {
        match guest_output.into_inner().try_into() {
            Ok(v) => v,
            Err(e) => ValidationPackageCallbackResult::Fail(format!("{:?}", e)),
        }
    }
}

impl CallbackResult for ValidationPackageCallbackResult {
    fn is_definitive(&self) -> bool {
        match self {
            ValidationPackageCallbackResult::Fail(_) => true,
            _ => false,
        }
    }
}
