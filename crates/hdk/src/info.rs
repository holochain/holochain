use crate::prelude::*;

/// Trivial wrapper for `__agent_info` host function.
/// Agent info input struct is `()` so the function call simply looks like this:
///
/// ```ignore
/// let agent_info = agent_info()?;
/// ```
///
/// the [ `AgentInfo` ] is the current agent's original pubkey/address that they joined the network with
/// and their most recent pubkey/address.
pub fn agent_info() -> ExternResult<AgentInfo> {
    host_call::<(), AgentInfo>(__agent_info, ())
}

/// @todo Not implemented
pub fn app_info() -> ExternResult<AppInfo> {
    host_call::<(), AppInfo>(__app_info, ())
}

/// @todo Not implemented
pub fn dna_info() -> ExternResult<DnaInfo> {
    host_call::<(), DnaInfo>(__dna_info, ())
}

/// Get the zome information.
/// There are no inputs to [ `zome_info` ].
///
/// Zome information includes dna name, hash, zome name and properties.
///
/// In general any holochain compatible wasm can be compiled and run in any zome so the zome info
/// needs to be looked up at runtime to e.g. know where to send/receive `call_remote` rpc calls to.
pub fn zome_info() -> ExternResult<ZomeInfo> {
    host_call::<(), ZomeInfo>(__zome_info, ())
}

/// @todo Not implemented
pub fn call_info() -> ExternResult<CallInfo> {
    host_call::<(), CallInfo>(__call_info, ())
}
