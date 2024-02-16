use crate::prelude::*;
pub use hdi::info::*;

/// Trivial wrapper for `__agent_info` host function.
/// Agent info input struct is `()` so the function call simply looks like this:
///
/// ```ignore
/// let agent_info = agent_info()?;
/// ```
///
/// the [AgentInfo] is the current agent's original pubkey/address that they joined the network with
/// and their most recent pubkey/address.
pub fn agent_info() -> ExternResult<AgentInfo> {
    HDK.with(|h| h.borrow().agent_info(()))
}

/// Get the context for a zome call, including the provenance and chain head.
///
/// See [CallInfo] for more details of what is returned.
/// 
/// Call info input struct is `()` so the function call simply looks like this:
///
/// ```ignore
/// let call_info = call_info()?;
/// ```
pub fn call_info() -> ExternResult<CallInfo> {
    HDK.with(|h| h.borrow().call_info(()))
}
