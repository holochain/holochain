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
pub fn call<'a, I: 'a, O>(
    to_cell: Option<CellId>,
    zome_name: ZomeName,
    fn_name: FunctionName,
    cap_secret: Option<CapSecret>,
    payload: &'a I,
) -> HdkResult<O>
where
    SerializedBytes: TryFrom<&'a I, Error = SerializedBytesError>,
    O: TryFrom<SerializedBytes, Error = SerializedBytesError>,
{
    let payload = SerializedBytes::try_from(payload)?;
    // @todo is this secure to set this in the wasm rather than have the host inject it?
    let provenance = agent_info()?.agent_latest_pubkey;
    let out = host_fn!(
        __call,
        CallInput::new(Call::new(
            to_cell, zome_name, fn_name, cap_secret, payload, provenance
        )),
        CallOutput
    )?;
    match out {
        ZomeCallResponse::Ok(o) => Ok(O::try_from(o.into_inner())?),
        ZomeCallResponse::Unauthorized => Err(HdkError::UnauthorizedZomeCall),
    }
}
