use crate::element::Element;
use crate::CallbackResult;
use holo_hash::AnyDhtHash;
use holochain_serialized_bytes::prelude::*;
use holochain_wasmer_common::*;

/// The validation status for an op or element
/// much of this happens in the subconscious
/// an entry missing validation dependencies may cycle through Pending many times before finally
/// reaching a final validation state or being abandoned

#[derive(
    Clone, Copy, Hash, serde::Serialize, serde::Deserialize, PartialOrd, Ord, Debug, Eq, PartialEq,
)]
#[cfg_attr(feature = "full", derive(num_enum::TryFromPrimitive))]
#[cfg_attr(feature = "full", repr(i32))]
pub enum ValidationStatus {
    /// all implemented validation callbacks found all dependencies and passed validation
    Valid = 0,
    /// some implemented validation callback definitively failed validation
    Rejected = 1,
    /// the subconscious has decided to never again attempt a conscious validation
    /// commonly due to missing validation dependencies remaining missing for "too long"
    Abandoned = 2,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub struct ValidateData {
    pub element: Element,
    pub validation_package: Option<ValidationPackage>,
}

impl ValidateData {
    pub fn new(element: Element, validation_package: Option<ValidationPackage>) -> Self {
        Self {
            element,
            validation_package,
        }
    }

    pub fn new_element_only(element: Element) -> Self {
        Self {
            element,
            validation_package: None,
        }
    }
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
    fn try_from_wasm_error(wasm_error: WasmError) -> Result<Self, WasmError> {
        match wasm_error.error {
            WasmErrorInner::Guest(_)
            | WasmErrorInner::Serialize(_)
            | WasmErrorInner::Deserialize(_) => {
                Ok(ValidateCallbackResult::Invalid(wasm_error.to_string()))
            }
            WasmErrorInner::Host(_)
            | WasmErrorInner::HostShortCircuit(_)
            | WasmErrorInner::GuestResultHandling(_)
            | WasmErrorInner::Compile(_)
            | WasmErrorInner::CallError(_)
            | WasmErrorInner::PointerMap
            | WasmErrorInner::ErrorWhileError
            | WasmErrorInner::Memory => Err(wasm_error),
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

impl CallbackResult for ValidationPackageCallbackResult {
    fn is_definitive(&self) -> bool {
        matches!(self, ValidationPackageCallbackResult::Fail(_))
    }
    fn try_from_wasm_error(wasm_error: WasmError) -> Result<Self, WasmError> {
        match wasm_error.error {
            WasmErrorInner::Guest(_)
            | WasmErrorInner::Serialize(_)
            | WasmErrorInner::Deserialize(_) => Ok(ValidationPackageCallbackResult::Fail(
                wasm_error.to_string(),
            )),
            WasmErrorInner::Host(_)
            | WasmErrorInner::HostShortCircuit(_)
            | WasmErrorInner::GuestResultHandling(_)
            | WasmErrorInner::Compile(_)
            | WasmErrorInner::CallError(_)
            | WasmErrorInner::PointerMap
            | WasmErrorInner::ErrorWhileError
            | WasmErrorInner::Memory => Err(wasm_error),
        }
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

#[cfg(feature = "full")]
impl rusqlite::ToSql for ValidationStatus {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput> {
        Ok(rusqlite::types::ToSqlOutput::Owned((*self as i32).into()))
    }
}

#[cfg(feature = "full")]
impl rusqlite::types::FromSql for ValidationStatus {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        i32::column_result(value).and_then(|int| {
            Self::try_from(int).map_err(|_| rusqlite::types::FromSqlError::InvalidType)
        })
    }
}
