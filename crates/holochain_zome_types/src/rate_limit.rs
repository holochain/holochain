//! The result of the `weigh` callback

use crate::CallbackResult;
use holochain_integrity_types::rate_limit::{RateBucketId, RateWeight};
use holochain_serialized_bytes::prelude::*;
use holochain_wasmer_common::WasmError;

pub use holochain_integrity_types::rate_limit::*;

/// The result of the `weigh` callback
#[derive(Clone, PartialEq, Serialize, Deserialize, SerializedBytes, Debug)]
pub struct WeighCallbackResult {
    /// The ascribed bucket id
    pub rate_bucket: RateBucketId,
    /// The ascribed weight
    pub rate_weight: RateWeight,
}

impl CallbackResult for WeighCallbackResult {
    fn is_definitive(&self) -> bool {
        true
    }
    fn try_from_wasm_error(wasm_error: WasmError) -> Result<Self, WasmError> {
        Err(wasm_error)
    }
}

impl Default for WeighCallbackResult {
    fn default() -> Self {
        Self {
            rate_bucket: 255,
            rate_weight: 0,
        }
    }
}
