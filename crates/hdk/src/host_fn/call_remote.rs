use crate::prelude::*;

/// Wrapper for __call_remote host function.
///
/// There are several positional arguments to the macro:
///
/// - agent: The address of the agent to call the RPC style remote function on.
/// - zome: The zome to call the remote function in. Use zome_info() to get the current zome info.
/// - fn_name: The name of the function in the zome to call.
/// - cap_secret: Optional cap claim secret to allow access to the remote call.
/// - payload: The payload to send to the remote function; receiver needs to deserialize cleanly.
///
/// Response is ZomeCallResponse which can either return ZomeCallResponse::Ok or
/// ZomeCallResponse::Unauthorized if the provided cap grant is invalid. The Unauthorized case
/// should always be handled gracefully because gap grants can be revoked at any time and the claim
/// holder has no way of knowing until they provide a secret for a call.
///
/// An Ok response includes `SerializedBytes` because the HDK doesn't know anything about the
/// function on the other end, even if it is the same zome, so you need to provide a structure that
/// will deserialize the result correctly.
///
/// The easiest way to do this is to create a shared crate that includes all the shared types for
/// cross-zome logic.
///
/// ```ignore
/// let serialized_bytes: SerializedBytes = match call_remote(bob, "foo_zome", "do_it", secret, serialized_payload)? {
///   ZomeCallResponse::Ok(sb) => sb,
///   ZomeCallResponse::Unauthorized => ...,
/// };
/// let deserialized_thing: SharedThing = serialized_bytes.try_into()?;
/// ```
pub fn call_remote(
    agent: AgentPubKey,
    zome: ZomeName,
    fn_name: FunctionName,
    cap_secret: Option<CapSecret>,
    payload: SerializedBytes,
) -> HdkResult<ZomeCallResponse> {
    host_fn!(
        __call_remote,
        CallRemoteInput::new(CallRemote::new(agent, zome, fn_name, cap_secret, payload)),
        CallRemoteOutput
    )
}
