//! Emit an app-defined Signal.
//!
//! Only clients who have subscribed to signals from this Cell with the proper
//! filters will receive it.

#[macro_export]
macro_rules! emit_signal {
    // TODO: we could consider adding a (optional?) "type" parameter, so that
    // statically typed languages can more easily get a hint of what type to
    // deserialize to. This of course requires a corresponding change to the
    // Signal type.
    ( $data:expr ) => {{
        let sb = $crate::prelude::SerializedBytes::try_from($data)?;
        $crate::host_fn!(
            __emit_signal,
            $crate::prelude::EmitSignalInput::new(sb),
            $crate::prelude::EmitSignalOutput
        )
    }};
}
