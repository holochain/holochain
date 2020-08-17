#[macro_export]
macro_rules! debug {
    ( $msg:expr ) => {
        $crate::debug!( "{}", $msg );
    };
    ( $msg:expr, $($tail:expr),* ) => {{
        $crate::host_fn!(
            __debug,
            $crate::prelude::DebugInput::new($crate::prelude::debug_msg!($msg, $($tail),*)),
            $crate::prelude::DebugOutput
        )
    }};
}
