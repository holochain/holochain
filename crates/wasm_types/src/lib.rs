use holochain_serialized_bytes::prelude::*;
use core::time::Duration;

#[derive(Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
pub struct WasmExternResponse(SerializedBytes);

impl WasmExternResponse {
    pub fn new(serialized_bytes: SerializedBytes) -> Self {
        Self(serialized_bytes)
    }
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
pub struct DebugInput(String);

impl DebugInput {
    pub fn new(s: &str) -> Self {
        Self(s.to_string())
    }

    pub fn inner(&self) -> &str {
        &self.0
    }
}

pub type DebugOutput = ();

pub type GlobalsInput = ();
pub type GlobalsOutput = ();

pub type SysTimeInput = ();

#[derive(Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
pub struct SysTimeOutput(Duration);

impl SysTimeOutput {
    pub fn new(duration: Duration) -> Self {
        Self(duration)
    }
}
