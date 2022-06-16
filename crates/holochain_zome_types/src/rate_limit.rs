//! Types for rate limiting, including the result of the `weigh` callback

use crate::CallbackResult;
use holochain_serialized_bytes::prelude::*;
use holochain_wasmer_common::WasmError;

pub use holochain_integrity_types::rate_limit::*;

/// The result of the `weigh` callback
#[derive(Clone, PartialEq, Serialize, Deserialize, SerializedBytes, Debug, Default)]
pub struct RateLimitsCallbackResult(Vec<RateLimit>);

impl RateLimitsCallbackResult {
    /// Constructor
    pub fn new(buckets: Vec<RateLimit>) -> Self {
        Self(buckets)
    }
}

impl From<Vec<RateLimit>> for RateLimitsCallbackResult {
    fn from(buckets: Vec<RateLimit>) -> Self {
        Self::new(buckets)
    }
}

impl CallbackResult for RateLimitsCallbackResult {
    fn is_definitive(&self) -> bool {
        true
    }
    fn try_from_wasm_error(wasm_error: WasmError) -> Result<Self, WasmError> {
        Err(wasm_error)
    }
}
