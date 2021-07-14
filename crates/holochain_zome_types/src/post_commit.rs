use crate::header::HeaderHashes;
use crate::CallbackResult;
use holochain_serialized_bytes::prelude::*;
use holochain_wasmer_common::WasmError;

#[derive(Clone, PartialEq, Serialize, Deserialize, SerializedBytes, Debug)]
pub enum PostCommitCallbackResult {
    Success,
    Fail(HeaderHashes, String),
}

impl CallbackResult for PostCommitCallbackResult {
    fn is_definitive(&self) -> bool {
        matches!(self, PostCommitCallbackResult::Fail(_, _))
    }
    fn try_from_wasm_error(wasm_error: WasmError) -> Result<Self, WasmError> {
        // Actually we don't map failures in post commit to a Fail.
        // That needs to be handled explicitly against header hashes.
        Err(wasm_error)
    }
}
