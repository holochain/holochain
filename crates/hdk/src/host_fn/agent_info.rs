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
