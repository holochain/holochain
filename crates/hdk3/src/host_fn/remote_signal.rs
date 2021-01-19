//! Emit an app-defined Signal remotely on a list of agents.

use crate::prelude::*;

/// ## Remote Signal
/// Send a signal to a list of other agents.
/// This will send the data as an [AppSignal] to
/// this zome for all the agents supplied.
///
/// ### Non-blocking
/// This is a non-blocking call and will not return an
/// error if the calls fail. This is designed to be used
/// as a send and forget operation.
/// A log will be produced at `[remote_signal]=info` if the calls
/// fail though (this may be removed in the future).
///
/// ### Usage
/// Currently this requires the function `recv_remote_signal` be
/// exposed by this zome with a signature like:
/// ```ignore
/// #[hdk_extern]
/// fn recv_remote_signal(signal: SerializedBytes) -> ExternResult<()> {
///     emit_signal(&signal)?;
///     Ok(())
/// }
/// ```
/// This function will also need to be added to your init as a
/// unrestricted cap grant so it can be called remotely.
///
/// This requirements will likely be removed in the future as
/// we design a better way to grant the capability to remote signal.
pub fn remote_signal<I>(input: I, agents: Vec<AgentPubKey>) -> ExternResult<()>
where
    I: serde::Serialize,
{
    #[allow(clippy::unit_arg)]
    host_call::<RemoteSignal, ()>(
        __remote_signal,
        RemoteSignal {
            signal: ExternIO::encode(input)?,
            agents,
        },
    )
}
