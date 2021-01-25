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
pub fn call<I>(
    to_cell: Option<CellId>,
    zome_name: ZomeName,
    fn_name: FunctionName,
    cap_secret: Option<CapSecret>,
    payload: I,
) -> ExternResult<ZomeCallResponse>
where
    I: serde::Serialize + std::fmt::Debug,
{
    // @todo is this secure to set this in the wasm rather than have the host inject it?
    let provenance = agent_info()?.agent_latest_pubkey;
    host_call::<Call, ZomeCallResponse>(
        __call,
        Call::new(
            to_cell,
            zome_name,
            fn_name,
            cap_secret,
            ExternIO::encode(payload).map_err(|e| {
                WasmError::new(
                    WasmErrorType::Serialize(e),
                    "Failed to serialize the payload for a call to the host.",
                )
            })?,
            provenance,
        ),
    )
}
