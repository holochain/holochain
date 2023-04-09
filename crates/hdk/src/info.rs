use crate::prelude::*;
pub use hdi::info::*;

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
    HDK.with(|h| h.borrow().agent_info(()))
}

/// Trivial wrapper for `__agent_info` host function.
/// Call info input struct is `()` so the function call simply looks like this:
///
/// ```ignore
/// let call_info = call_info()?;
/// ```
///
/// the [ `CallInfo` ] is
/// - the provenance of the call
/// - function name that was the extern/entrypoint into the wasm
/// - the chain head as at the start of the call, won't change even if the chain
///   is written to during the call
/// - the [ `CapGrant` ] used to authorize the call
pub fn call_info() -> ExternResult<CallInfo> {
    HDK.with(|h| h.borrow().call_info(()))
}
