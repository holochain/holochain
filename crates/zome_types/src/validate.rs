use crate::element::Element;
use crate::zome_io::ExternOutput;
use crate::CallbackResult;
use holo_hash::AnyDhtHash;
use holochain_serialized_bytes::prelude::*;

#[derive(Serialize, Deserialize, SerializedBytes)]
pub struct ValidateData {
    pub element: Element,
    pub validation_package: Option<ValidationPackage>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidateCallbackResult {
    Valid,
    Invalid(String),
    /// Subconscious needs to map this to either pending or abandoned based on context that the
    /// wasm can't possibly have.
    UnresolvedDependencies(Vec<AnyDhtHash>),
}

impl CallbackResult for ValidateCallbackResult {
    fn is_definitive(&self) -> bool {
        match self {
            ValidateCallbackResult::Invalid(_) => true,
            _ => false,
        }
    }
}

impl From<ExternOutput> for ValidateCallbackResult {
    fn from(guest_output: ExternOutput) -> Self {
        match guest_output.into_inner().try_into() {
            Ok(v) => v,
            Err(e) => Self::Invalid(format!("{:?}", e)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub struct ValidationPackage;

/// The level of validation package required by
/// an entry.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum RequiredValidationPackage {
    /// Just the element (default)
    Element,
    /// N number of chain elements counting back from
    /// this entry
    Chain(usize),
    /// The entire chain
    Full,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidationPackageCallbackResult {
    Success(ValidationPackage),
    Fail(String),
    UnresolvedDependencies(Vec<AnyDhtHash>),
}

impl From<ExternOutput> for ValidationPackageCallbackResult {
    fn from(guest_output: ExternOutput) -> Self {
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

impl Default for RequiredValidationPackage {
    fn default() -> Self {
        Self::Element
    }
}
