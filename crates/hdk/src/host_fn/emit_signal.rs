//! Emit an app-defined Signal.
//!
//! Only clients who have subscribed to signals from this Cell with the proper
//! filters will receive it.

#[macro_export]
macro_rules! emit_signal {
    ( $data:expr ) => {{
        let sb = $crate::prelude::SerializedBytes::try_from($data)?;
        $crate::host_fn!(
            __emit_signal,
            $crate::prelude::EmitSignalInput::new(sb),
            $crate::prelude::EmitSignalOutput
        )
    }};
}
