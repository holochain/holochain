/// Debug anything that can be formatted.
///
/// Internally calls trace_msg! which should preserve the line numbers etc. from _inside the wasm_
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
        $crate::debug!("{}", $msg );
    };
    ( $msg:expr, $($tail:expr),* ) => {{
        host_call::<crate::prelude::TraceMsg, ()>(
            __trace,
            holochain_zome_types::trace_msg!(holochain_zome_types::trace::Level::DEBUG, $msg, $($tail),*),
        ).ok();
    }};
}

#[macro_export]
macro_rules! trace {
    ( $msg:expr ) => {
        $crate::trace!("{}", $msg );
    };
    ( $msg:expr, $($tail:expr),* ) => {{
        host_call::<crate::prelude::TraceMsg, ()>(
            __trace,
            holochain_zome_types::trace_msg!(holochain_zome_types::trace::Level::TRACE, $msg, $($tail),*),
        ).ok();
    }};
}

#[macro_export]
macro_rules! info {
    ( $msg:expr ) => {
        $crate::info!("{}", $msg );
    };
    ( $msg:expr, $($tail:expr),* ) => {{
        host_call::<crate::prelude::TraceMsg, ()>(
            __trace,
            holochain_zome_types::trace_msg!(holochain_zome_types::trace::Level::INFO, $msg, $($tail),*),
        ).ok();
    }};
}

#[macro_export]
macro_rules! warn {
    ( $msg:expr ) => {
        $crate::warn!("{}", $msg );
    };
    ( $msg:expr, $($tail:expr),* ) => {{
        host_call::<crate::prelude::TraceMsg, ()>(
            __trace,
            holochain_zome_types::trace_msg!(holochain_zome_types::trace::Level::WARN, $msg, $($tail),*),
        ).ok();
    }};
}

#[macro_export]
macro_rules! error {
    ( $msg:expr ) => {
        $crate::error!("{}", $msg );
    };
    ( $msg:expr, $($tail:expr),* ) => {{
        host_call::<crate::prelude::TraceMsg, ()>(
            __trace,
            holochain_zome_types::trace_msg!(holochain_zome_types::trace::Level::ERROR, $msg, $($tail),*),
        ).ok();
    }};
}