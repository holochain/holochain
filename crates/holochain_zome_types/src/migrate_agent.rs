use crate::CallbackResult;
use holochain_serialized_bytes::prelude::*;
use holochain_wasmer_common::WasmError;

#[derive(Clone, Serialize, Deserialize, SerializedBytes, Debug)]
pub enum MigrateAgent {
    Open,
    Close,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, SerializedBytes, Debug)]
pub enum MigrateAgentCallbackResult {
    Pass,
    Fail(String),
}

impl CallbackResult for MigrateAgentCallbackResult {
    fn is_definitive(&self) -> bool {
        matches!(self, MigrateAgentCallbackResult::Fail(_))
    }
    fn try_from_wasm_error(wasm_error: WasmError) -> Result<Self, WasmError> {
        match wasm_error {
            WasmError::Guest(_) | WasmError::Serialize(_) | WasmError::Deserialize(_) => {
                Ok(MigrateAgentCallbackResult::Fail(wasm_error.to_string()))
            }
            _ => Err(wasm_error),
        }
    }
}
