#[macro_export]
macro_rules! sys_time {
    () => {{
        $crate::host_fn!(
            __sys_time,
            $crate::prelude::SysTimeInput::new(()),
            $crate::prelude::SysTimeOutput
        )
    }};
}
