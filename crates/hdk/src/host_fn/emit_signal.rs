//! Emit an app-defined Signal.
//!
//! Only clients who have subscribed to signals from this Cell with the proper
//! filters will receive it.

use crate::prelude::*;

// TODO: we could consider adding a (optional?) "type" parameter, so that
// statically typed languages can more easily get a hint of what type to
// deserialize to. This of course requires a corresponding change to the
// Signal type.
pub fn emit_signal<'a, D: 'a>(data: &'a D) -> HdkResult<()>
where
    SerializedBytes: TryFrom<&'a D, Error = SerializedBytesError>,
{
    let sb = SerializedBytes::try_from(data)?;
    host_fn!(__emit_signal, EmitSignalInput::new(sb), EmitSignalOutput)
}
