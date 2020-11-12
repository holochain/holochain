use crate::prelude::*;
use holochain_wasmer_guest::SerializedBytesError;
use holochain_zome_types::{
    call::Call,
    zome::{FunctionName, ZomeName},
};

use crate::host_fn;

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
/// - request: The arguments to the function you are calling.
pub fn call<I, O>(
    to_cell: Option<CellId>,
    zome_name: ZomeName,
    fn_name: FunctionName,
    cap_secret: Option<CapSecret>,
    request: I,
) -> ExternResult<O>
where
    I: TryInto<SerializedBytes, Error = SerializedBytesError>,
    O: TryFrom<SerializedBytes, Error = SerializedBytesError>,
{
    let request: SerializedBytes = request.try_into()?;
    let provenance = agent_info()?.agent_latest_pubkey;
    let out = host_fn!(
        __call,
        CallInput::new(Call::new(
            to_cell, zome_name, fn_name, cap_secret, request, provenance
        )),
        CallOutput
    )?;
    match out {
        ZomeCallResponse::Ok(e) => Ok(O::try_from(e.into_inner())?),
        ZomeCallResponse::Unauthorized => Err(HdkError::UnauthorizedZomeCall),
    }
}
