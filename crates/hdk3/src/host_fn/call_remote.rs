use crate::prelude::*;

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
/// Response is HdkResult which can either return HdkResult::Ok with the deserialized result
/// of the function call, HdkError::ZomeCallNetworkError if there was a network error,
/// or HdkError::UnauthorizedZomeCall if the provided cap grant is invalid. The Unauthorized case
/// should always be handled gracefully because gap grants can be revoked at any time and the claim
/// holder has no way of knowing until they provide a secret for a call.
///
/// An Ok response already includes the deserialized type. Note that this type must be specified
/// should derive from `SerializedBytes`, and it should be the same one as the return type
/// from the remote function call.
///
///
/// ```ignore
/// #[derive(SerializedBytes, Serialize, Deserialize)]
/// struct Foo(String);
///
/// ...
/// let foo: Foo = call_remote(bob, "foo_zome", "do_it", secret, serialized_payload)?;
/// ...
/// ```
pub fn call_remote<'a, I: 'a, O>(
    agent: AgentPubKey,
    zome: ZomeName,
    fn_name: FunctionName,
    cap_secret: Option<CapSecret>,
    payload: &'a I,
) -> HdkResult<O>
where
    SerializedBytes: TryFrom<&'a I, Error = SerializedBytesError>,
    O: TryFrom<SerializedBytes, Error = SerializedBytesError>,
{
    let payload = SerializedBytes::try_from(payload)?;
    let out = host_call::<CallRemoteInput, CallRemoteOutput>(
        __call_remote,
        &CallRemoteInput::new(CallRemote::new(agent, zome, fn_name, cap_secret, payload)),
    )?
    .into_inner();

    match out {
        ZomeCallResponse::Ok(o) => Ok(O::try_from(o.into_inner())?),
        ZomeCallResponse::Unauthorized(c, z, f, p) => {
            Err(HdkError::UnauthorizedZomeCall(c, z, f, p))
        }
        ZomeCallResponse::NetworkError(e) => Err(HdkError::ZomeCallNetworkError(e)),
    }
}
