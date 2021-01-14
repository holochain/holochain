//! Emit an app-defined Signal.
//!
//! Only clients who have subscribed to signals from this Cell with the proper
//! filters will receive it.

use crate::prelude::*;
use holochain_zome_types::signal::AppSignal;

// TODO: we could consider adding a (optional?) "type" parameter, so that
// statically typed languages can more easily get a hint of what type to
// deserialize to. This of course requires a corresponding change to the
// Signal type.
// pub fn emit_signal<'a, D: 'a>(data: &'a D) -> HdkResult<()>
// where
//     SerializedBytes: TryFrom<&'a D, Error = SerializedBytesError>,
// {
//     let sb = SerializedBytes::try_from(data)?;
//     #[allow(clippy::unit_arg)]
//     Ok(host_call::<EmitSignalInput, EmitSignalOutput>(
//         __emit_signal,
//         &EmitSignalInput::new(AppSignal::new(sb)),
//     )?
//     .into_inner())
// }

pub fn emit_signal<D, E>(data: D) -> ExternResult<()>
where
    WasmError: From<E>,
    SerializedBytes: TryFrom<D, Error = E>,
{
    let sb = SerializedBytes::try_from(data)?;
    #[allow(clippy::unit_arg)]
    host_call::<AppSignal, ()>(__emit_signal, AppSignal::new(sb))
}
