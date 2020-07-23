use hdk3::prelude::*;

holochain_wasmer_guest::host_externs!(__call_remote);

fn _whoami(_: ()) -> Result<AgentInfo, WasmError> {
    Ok(agent_info!()?)
}

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
