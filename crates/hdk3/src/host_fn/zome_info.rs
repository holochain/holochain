use crate::prelude::*;

/// Get the zome information.
/// There are no inputs to zome_info.
///
/// Zome information includes dna name, hash, zome name and properties.
///
/// In general any holochain compatible wasm can be compiled and run in any zome so the zome info
/// needs to be looked up at runtime to e.g. know where to send/receive call_remote rpc calls to.
pub fn zome_info() -> HdkResult<ZomeInfo> {
    Ok(
        host_call::<ZomeInfoInput, ZomeInfoOutput>(__zome_info, ZomeInfoInput::new(()))?
            .into_inner(),
    )
}
