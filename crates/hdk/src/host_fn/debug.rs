#[macro_export]
macro_rules! debug {
    ( $msg:expr ) => {{
        $crate::host_fn!(
            __debug,
            $crate::prelude::DebugInput::new($crate::prelude::debug_msg!(format!("{:?}", $msg))),
            $crate::prelude::DebugOutput
        )
    }};
}
