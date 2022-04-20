pub use holochain_integrity_types::entry_def::*;

use crate::CallbackResult;
use holochain_wasmer_common::WasmError;

impl CallbackResult for EntryDefsCallbackResult {
    fn is_definitive(&self) -> bool {
        false
    }
    fn try_from_wasm_error(wasm_error: WasmError) -> Result<Self, WasmError> {
        // There is no concept of entry defs failing, other than normal error handling.
        Err(wasm_error)
    }
}
