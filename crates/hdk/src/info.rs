use crate::prelude::*;
pub use idk::info::*;

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

/// @todo Not implemented
pub fn call_info() -> ExternResult<CallInfo> {
    HDK.with(|h| h.borrow().call_info(()))
}
