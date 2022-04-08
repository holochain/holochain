use crate::entry::Entry;
use crate::header::CreateLink;
use crate::header::DeleteLink;
use crate::CallbackResult;
use holo_hash::AnyDhtHash;
use holochain_serialized_bytes::prelude::*;
use holochain_wasmer_common::*;

#[derive(Serialize, Deserialize, SerializedBytes, Debug)]
pub struct ValidateCreateLinkData {
    pub link_add: CreateLink,
    pub base: Entry,
    pub target: Entry,
}

#[derive(Serialize, Deserialize, SerializedBytes, Debug)]
pub struct ValidateDeleteLinkData {
    pub delete_link: DeleteLink,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidateLinkCallbackResult {
    Valid,
    Invalid(String),
    UnresolvedDependencies(Vec<AnyDhtHash>),
}

impl CallbackResult for ValidateLinkCallbackResult {
    fn is_definitive(&self) -> bool {
        matches!(self, ValidateLinkCallbackResult::Invalid(_))
    }
    fn try_from_wasm_error(wasm_error: WasmError) -> Result<Self, WasmError> {
        match wasm_error.error {
            WasmErrorInner::Guest(_)
            | WasmErrorInner::Serialize(_)
            | WasmErrorInner::Deserialize(_) => {
                Ok(ValidateLinkCallbackResult::Invalid(wasm_error.to_string()))
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
