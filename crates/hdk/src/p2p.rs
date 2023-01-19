use crate::prelude::*;

/// # Call
/// Make a Zome call in another Zome.
/// The Zome can be in another Cell or the
/// same Cell but must be installed on the same conductor.
///
/// ## Parameters
/// - to_cell: The cell you want to call (If None will call the current cell).
/// - zome_name: The name of the zome you want to call.
/// - fn_name: The name of the function in the zome you are calling.
/// - cap_secret: The capability secret if required.
/// - payload: The arguments to the function you are calling.
pub fn call<I, Z>(
    to_cell: CallTargetCell,
    zome_name: Z,
    fn_name: FunctionName,
    cap_secret: Option<CapSecret>,
    payload: I,
) -> ExternResult<ZomeCallResponse>
where
    I: serde::Serialize + std::fmt::Debug,
    Z: Into<ZomeName>,
{
    Ok(HDK
        .with(|h| {
            h.borrow().call(vec![Call::new(
                CallTarget::ConductorCell(to_cell),
                zome_name.into(),
                fn_name,
                cap_secret,
                ExternIO::encode(payload).map_err(|e| wasm_error!(e))?,
            )])
        })?
        .into_iter()
        .next()
        .unwrap())
}

/// Wrapper for __call_remote host function.
///
/// There are several positional arguments:
///
/// - agent: The address of the agent to call the RPC style remote function on.
/// - zome: The zome to call the remote function in. Use zome_info() to get the current zome info.
/// - fn_name: The name of the function in the zome to call.
/// - cap_secret: Optional cap claim secret to allow access to the remote call.
/// - payload: The payload to send to the remote function; receiver needs to deserialize cleanly.
///
/// Response is [ `ExternResult` ] which returns [ `ZomeCallResponse` ] of the function call.
/// [ `ZomeCallResponse::NetworkError` ] if there was a network error.
/// [ `ZomeCallResponse::Unauthorized` ] if the provided cap grant is invalid.
/// The unauthorized case should always be handled gracefully because gap grants can be revoked at
/// any time and the claim holder has no way of knowing until they provide a secret for a call.
///
/// An Ok response already includes an [ `ExternIO` ] to be deserialized with `extern_io.decode()?`.
///
/// ```ignore
/// ...
/// let foo: Foo = call_remote(bob, "foo_zome", "do_it", secret, serializable_payload)?;
/// ...
/// ```
pub fn call_remote<I, Z>(
    agent: AgentPubKey,
    zome: Z,
    fn_name: FunctionName,
    cap_secret: Option<CapSecret>,
    payload: I,
) -> ExternResult<ZomeCallResponse>
where
    I: serde::Serialize + std::fmt::Debug,
    Z: Into<ZomeName>,
{
    Ok(HDK
        .with(|h| {
            h.borrow().call(vec![Call::new(
                CallTarget::NetworkAgent(agent),
                zome.into(),
                fn_name,
                cap_secret,
                ExternIO::encode(payload).map_err(|e| wasm_error!(e))?,
            )])
        })?
        .into_iter()
        .next()
        .unwrap())
}

/// Emit an app-defined Signal.
///
/// Only clients who have subscribed to signals from this Cell with the proper
/// filters will receive it.
///
/// # Examples
/// <https://github.com/holochain/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/emit_signal/src/lib.rs>
//
// TODO: we could consider adding a (optional?) "type" parameter, so that
// statically typed languages can more easily get a hint of what type to
// deserialize to. This of course requires a corresponding change to the
// Signal type.
pub fn emit_signal<I>(input: I) -> ExternResult<()>
where
    I: serde::Serialize + std::fmt::Debug,
{
    HDK.with(|h| {
        h.borrow().emit_signal(AppSignal::new(
            ExternIO::encode(input).map_err(|e| wasm_error!(e))?,
        ))
    })
}

/// ## Remote Signal
/// Send a signal to a list of other agents.
/// This will send the data as an [ `AppSignal` ] to
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
    I: serde::Serialize + std::fmt::Debug,
{
    HDK.with(|h| {
        h.borrow().remote_signal(RemoteSignal {
            signal: ExternIO::encode(input).map_err(|e| wasm_error!(e))?,
            agents,
        })
    })
}
