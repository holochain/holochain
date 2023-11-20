//! externs to help bench the wasm ribosome

use hdk::prelude::*;

/// round trip bytes back to the host
/// useful to see what the basic throughput of our wasm implementation is
#[hdk_extern]
fn echo_bytes(bytes: Bytes) -> ExternResult<Bytes> {
    Ok(bytes)
}
