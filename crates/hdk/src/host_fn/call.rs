use crate::prelude::*;
use holo_hash::{AgentPubKey, DnaHash};
use holochain_wasmer_guest::SerializedBytesError;
use holochain_zome_types::{
    call::Call,
    zome::{FunctionName, ZomeName},
};

use crate::host_fn;

pub fn call<I, O>(
    to_agent: AgentPubKey,
    to_dna: Option<DnaHash>,
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
    let provenance = agent_info!()?.agent_latest_pubkey;
    let out = host_fn!(
        __call,
        CallInput::new(Call::new(
            to_agent, to_dna, zome_name, fn_name, cap_secret, request, provenance
        )),
        CallOutput
    )?;
    match out {
        ZomeCallResponse::Ok(e) => Ok(O::try_from(e.into_inner())?),
        ZomeCallResponse::Unauthorized => Err(HdkError::UnauthorizedZomeCall),
    }
}
