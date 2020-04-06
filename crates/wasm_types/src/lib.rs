use holochain_serialized_bytes::prelude::*;

#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
pub struct WasmExternResponse(SerializedBytes);

impl WasmExternResponse {
    pub fn new(serialized_bytes: SerializedBytes) -> Self {
        Self(serialized_bytes)
    }
}
