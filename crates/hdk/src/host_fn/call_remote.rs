/// Wrapper for __call_remote host function.
///
/// There are several positional arguments to the macro:
///
/// - agent: The address of the agent to call the RPC style remote function on.
/// - zome: The zome to call the remote function in. Use zome_info!() to get the current zome info.
/// - fn_name: The name of the function in the zome to call.
/// - request: The payload to send to the remote function; receiver needs to deserialize cleanly.
///
/// Response is ZomeCallResponse which can either because the HDK doesn't know anything about the function
/// on the other end, so you need to provide a structure that will deserialize the result correctly
/// the easiest way to do this is to create a shared crate that includes all the shared types for
/// cross-zome logic.
///
/// ```ignore
/// let serialized_bytes: SerializedBytes = call_remote!(bob, "foo_zome", "do_it", secret, serialized_payload);
/// let deserialized_thing: SharedThing = serialized_bytes.try_into()?;
/// ```
#[macro_export]
macro_rules! call_remote {
    ( $agent:expr, $zome:expr, $fn_name:expr, $cap:expr, $request:expr ) => {{
        $crate::host_fn!(
            __call_remote,
            $crate::prelude::CallRemoteInput::new($crate::prelude::CallRemote::new(
                $agent, $zome, $fn_name, $cap, $request
            )),
            $crate::prelude::CallRemoteOutput
        )
    }};
}
