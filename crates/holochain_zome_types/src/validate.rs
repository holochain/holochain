use std::collections::HashMap;
use crate::prelude::*;
use holochain_wasmer_common::*;

pub use holochain_integrity_types::validate::*;

/// The validation status for an op or record
/// much of this happens in the subconscious
/// an entry missing validation dependencies may cycle through Pending many times before finally
/// reaching a final validation state or being abandoned

#[derive(
    Clone, Copy, Hash, serde::Serialize, serde::Deserialize, PartialOrd, Ord, Debug, Eq, PartialEq,
)]
#[cfg_attr(feature = "full", derive(num_enum::TryFromPrimitive))]
#[cfg_attr(feature = "full", repr(i32))]
pub enum ValidationStatus {
    /// All dependencies were found and validation passed
    Valid = 0,
    /// Item was rejected by validation
    Rejected = 1,
    /// Holochain has decided to never again attempt validation,
    /// commonly due to missing validation dependencies remaining missing for "too long"
    Abandoned = 2,
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
            | WasmErrorInner::Compile(_)
            | WasmErrorInner::CallError(_)
            | WasmErrorInner::PointerMap
            | WasmErrorInner::ErrorWhileError
            | WasmErrorInner::Memory
            | WasmErrorInner::UninitializedSerializedModuleCache => Err(wasm_error),
        }
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

pub struct ValidationReceiptInfo {
    /// the result of the validation.
    pub validation_status: ValidationStatus,

    /// the remote validators who signed the receipt.
    pub validators: Vec<AgentPubKey>,

    /// The op hash that this receipt is for.
    pub op_hash: DhtOpHash,

    /// The type of the op that was validated.
    ///
    /// Note that the original type is discarded here because DhtOpType is part of `holochain_types`
    /// and moving it would be a breaking change. For now this is just informational.
    pub op_type: String,

    /// Whether this op has received the required number of receipts.
    pub receipts_complete: bool,
}
