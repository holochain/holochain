use hdk3::prelude::*;

holochain_wasmer_guest::host_externs!(__call_remote);

// returns the current agent info
fn _whoami(_: ()) -> Result<AgentInfo, WasmError> {
    Ok(agent_info!()?)
}

// returns the agent info reported by the given pub key
// in theory the output is the same as the input
// it's just that the output comes _from the opinion of the remote agent_
fn _whoarethey(agent_pubkey: AgentPubKey) -> Result<AgentInfo, WasmError> {
    let result: SerializedBytes = call_remote!(
        agent_pubkey,
        zome_info!()?.zome_name,
        "whoami".to_string(),
        CapSecret::default(),
        ().try_into()?
    )?;

    Ok(result.try_into()?)
}

map_extern!(whoami, _whoami);
map_extern!(whoarethey, _whoarethey);
