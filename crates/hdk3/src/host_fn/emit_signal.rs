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
pub fn emit_signal<I>(input: I) -> ExternResult<()>
where
    I: serde::Serialize + std::fmt::Debug,
{
    #[allow(clippy::unit_arg)]
    host_call::<AppSignal, ()>(
        __emit_signal,
        AppSignal::new(ExternIO::encode(input).map_err(|e| {
            WasmError::new(
                WasmErrorType::Serialize(e),
                "Failed to serialize the payload to emit a signal.",
            )
        })?),
    )
}
