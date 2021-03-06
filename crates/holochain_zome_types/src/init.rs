use crate::CallbackResult;
use holo_hash::AnyDhtHash;
use holochain_serialized_bytes::prelude::*;
use holochain_wasmer_common::WasmError;

#[derive(Clone, PartialEq, Serialize, Deserialize, SerializedBytes, Debug)]
pub enum InitCallbackResult {
    Pass,
    Fail(String),
    UnresolvedDependencies(Vec<AnyDhtHash>),
}

impl CallbackResult for InitCallbackResult {
    fn is_definitive(&self) -> bool {
        matches!(self, InitCallbackResult::Fail(_))
    }
    fn try_from_wasm_error(wasm_error: WasmError) -> Result<Self, WasmError> {
        match wasm_error {
            WasmError::Guest(_) | WasmError::Serialize(_) | WasmError::Deserialize(_) => {
                Ok(InitCallbackResult::Fail(wasm_error.to_string()))
            }
            WasmError::Host(_)
            | WasmError::HostShortCircuit(_)
            | WasmError::GuestResultHandling(_)
            | WasmError::Compile(_)
            | WasmError::CallError(_)
            | WasmError::PointerMap
            | WasmError::ErrorWhileError
            | WasmError::Memory => Err(wasm_error),
        }
    }
}
