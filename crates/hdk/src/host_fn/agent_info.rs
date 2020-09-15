/// Trivial macro wrapper for __agent_info host function.
/// Agent info input struct is `()` so the macro simply looks like this:
///
/// ```ignore
/// let agent_info = agent_info!()?;
/// ```
///
/// the AgentInfo is the current agent's original pubkey/address that they joined the network with
/// and their most recent pubkey/address.
#[macro_export]
macro_rules! agent_info {
    () => {{
        $crate::host_fn!(
            __agent_info,
            $crate::prelude::AgentInfoInput::new(()),
            $crate::prelude::AgentInfoOutput
        )
    }};
}
