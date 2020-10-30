use crate::prelude::*;
use holo_hash::AgentPubKey;
use holochain_wasmer_guest::SerializedBytesError;
use holochain_zome_types::{
    call::Call,
    zome::{FunctionName, ZomeName},
};

use crate::host_fn;

pub fn call(
    to_agent: AgentPubKey,
    zome_name: ZomeName,
    fn_name: FunctionName,
    cap_secret: Option<CapSecret>,
    request: SerializedBytes,
) -> Result<ZomeCallResponse, SerializedBytesError> {
    let provenance = agent_info!()?.agent_latest_pubkey;
    host_fn!(
        __call,
        CallInput::new(Call::new(
            to_agent, zome_name, fn_name, cap_secret, request, provenance
        )),
        CallOutput
    )
}
