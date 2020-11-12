use hdk3::prelude::*;

#[hdk_extern]
fn set_access(_: ()) -> ExternResult<()> {
    let mut functions: GrantedFunctions = HashSet::new();
    functions.insert((zome_info()?.zome_name, "whoami".into()));
    create_cap_grant(CapGrantEntry {
        tag: "".into(),
        // empty access converts to unrestricted
        access: ().into(),
        functions,
    })?;

    Ok(())
}

// returns the current agent info
#[hdk_extern]
fn whoami(_: ()) -> ExternResult<AgentInfo> {
    Ok(agent_info()?)
}

// returns the agent info reported by the given pub key
// in theory the output is the same as the input
// it's just that the output comes _from the opinion of the remote agent_
#[hdk_extern]
fn whoarethey(agent_pubkey: AgentPubKey) -> ExternResult<AgentInfo> {
    let response: ZomeCallResponse = call_remote(
        agent_pubkey,
        zome_info()?.zome_name,
        "whoami".to_string().into(),
        None,
        &(),
    )?;

    match response {
        ZomeCallResponse::Ok(guest_output) => Ok(guest_output.into_inner().try_into()?),
        // we're just panicking here because our simple tests can always call set_access before
        // calling whoami, but in a real app you'd want to handle this by returning an `Ok` with
        // something meaningful to the extern's client
        ZomeCallResponse::Unauthorized => unreachable!(),
    }
}

// returns the agent info reported by the given pub key
// in theory the output is the same as the input
// it's just that the output comes _from the opinion of the remote agent_
#[hdk_extern]
fn who_are_they_local(cell_id: CellId) -> ExternResult<AgentInfo> {
    call(
        Some(cell_id),
        zome_info()?.zome_name,
        "whoami".to_string().into(),
        None,
        &(),
    )
}

/// Call the create entry zome from this zome.
/// The cell id must point to a cell which includes
/// the "create_entry" zome.
#[hdk_extern]
fn call_create_entry(cell_id: CellId) -> ExternResult<HeaderHash> {
    Ok(call(
        Some(cell_id),
        "create_entry".to_string().into(),
        "create_entry".to_string().into(),
        None,
        &(),
    )?)
}
