//! externs to help bench the wasm ribosome

use hdk3::prelude::*;

holochain_externs!();

map_extern!(echo_bytes, _echo_bytes);

/// round trip bytes back to the host
/// useful to see what the basic throughput of our wasm implementation is
fn _echo_bytes(sb: SerializedBytes) -> Result<SerializedBytes, WasmError> {
    Ok(sb)
}
