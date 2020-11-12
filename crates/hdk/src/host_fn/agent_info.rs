use crate::prelude::*;

/// Trivial wrapper for __agent_info host function.
/// Agent info input struct is `()` so the macro simply looks like this:
///
/// ```ignore
/// let agent_info = agent_info()?;
/// ```
///
/// the AgentInfo is the current agent's original pubkey/address that they joined the network with
/// and their most recent pubkey/address.
pub fn agent_info() -> HdkResult<AgentInfo> {
    host_fn!(__agent_info, AgentInfoInput::new(()), AgentInfoOutput)
}
