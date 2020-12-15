//! Emit an app-defined Signal remotely on a list of agents.
//!
//! Only clients who have subscribed to signals from this Cell with the proper
//! filters will receive it.

use crate::prelude::*;
use holochain_zome_types::signal::AppSignal;

pub fn remote_signal<'a, D: 'a>(data: &'a D, agents: Vec<AgentPubKey>) -> HdkResult<()>
where
    SerializedBytes: TryFrom<&'a D, Error = SerializedBytesError>,
{
    let sb = SerializedBytes::try_from(data)?;
    #[allow(clippy::unit_arg)]
    Ok(host_call::<RemoteSignalInput, RemoteSignalOutput>(
        __remote_signal,
        &RemoteSignalInput::new(RemoteSignal {
            signal: AppSignal::new(sb),
            agents,
        }),
    )?
    .into_inner())
}
