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
        host_call::<DebugInput, DebugOutput>(
            __debug,
            $crate::prelude::DebugInput::new(holochain_zome_types::debug_msg!($msg, $($tail),*)),
        ).ok();
    }};
}
