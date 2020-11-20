/// Debug anything that can be formatted.
///
/// Internally calls debug_msg! which should preserve the line numbers etc. from _inside the wasm_
/// which is normally difficult information to access due to limited debugging options for wasm
/// code.
///
/// Note: Debugging happens _on the host side_ with the debug! macro from the tracing crate.
///
/// Note: Debug does not return a result.
///
/// ```ignore
/// debug!("{:?}", foo);
/// ```
#[macro_export]
macro_rules! debug {
    ( $msg:expr ) => {
        $crate::debug!( "{}", $msg );
    };
    ( $msg:expr, $($tail:expr),* ) => {{
        // We consume the result of debug!() inline because it doesn't mean anything to handle the
        // result of a debug. Technically there is a Result that represents deserialization from
        // the host, but the only thing the host is passing back to us is a hardcoded `Ok(())`.
        $crate::host_fn!(
            __debug,
            $crate::prelude::DebugInput::new($crate::prelude::debug_msg!($msg, $($tail),*)),
            $crate::prelude::DebugOutput
        ).ok();
    }};
}
