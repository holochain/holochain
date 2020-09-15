/// Debug anything that can be formatted.
///
/// Internally calls debug_msg! which should preserve the line numbers etc. from _inside the wasm_
/// which is normally difficult information to access due to limited debugging options for wasm
/// code.
///
/// Note: Debugging happens _on the host side_ with the debug! macro from the tracing crate.
///
/// Note: Debug returns a result like every host_fn so use `?` or `ok()` to handle it.
///
/// ```ignore
/// debug!("{:?}", foo)?;
/// debug!("{:?}", foo).ok();
/// ```
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
