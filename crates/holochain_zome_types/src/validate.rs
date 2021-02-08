use crate::element::Element;
use crate::zome_io::ExternIO;
use crate::CallbackResult;
use holo_hash::AnyDhtHash;
use holochain_serialized_bytes::prelude::*;

/// The validation status for an op or element
/// much of this happens in the subconscious
/// an entry missing validation dependencies may cycle through Pending many times before finally
/// reaching a final validation state or being abandoned
#[derive(
    Clone, Copy, Hash, serde::Serialize, serde::Deserialize, PartialOrd, Ord, Debug, Eq, PartialEq,
)]
pub enum ValidationStatus {
    /// all implemented validation callbacks found all dependencies and passed validation
    Valid,
    /// some implemented validation callback definitively failed validation
    Rejected,
    /// the subconscious has decided to never again attempt a conscious validation
    /// commonly due to missing validation dependencies remaining missing for "too long"
    Abandoned,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
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
        matches!(self, ValidateCallbackResult::Invalid(_))
    }
}

impl From<ExternIO> for ValidateCallbackResult {
    fn from(guest_output: ExternIO) -> Self {
        match guest_output.decode() {
            Ok(v) => v,
            Err(e) => Self::Invalid(format!("{:?}", e)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub struct ValidationPackage(pub Vec<Element>);

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

#[derive(Clone, PartialEq, Serialize, Deserialize, SerializedBytes, Debug)]
pub enum ValidationPackageCallbackResult {
    Success(ValidationPackage),
    Fail(String),
    UnresolvedDependencies(Vec<AnyDhtHash>),
}

impl From<ExternIO> for ValidationPackageCallbackResult {
    fn from(guest_output: ExternIO) -> Self {
        match guest_output.decode() {
            Ok(v) => v,
            Err(e) => ValidationPackageCallbackResult::Fail(format!("{:?}", e)),
        }
    }
}

impl CallbackResult for ValidationPackageCallbackResult {
    fn is_definitive(&self) -> bool {
        matches!(self, ValidationPackageCallbackResult::Fail(_))
    }
}

impl Default for RequiredValidationType {
    fn default() -> Self {
        Self::Element
    }
}

impl ValidationPackage {
    pub fn new(elements: Vec<Element>) -> Self {
        Self(elements)
    }
}
