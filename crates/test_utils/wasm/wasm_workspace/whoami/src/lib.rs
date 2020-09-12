use hdk3::prelude::*;

#[hdk_extern]
fn set_access(_: ()) -> ExternResult<()> {
    let mut functions: GrantedFunctions = HashSet::new();
    functions.insert((zome_info!()?.zome_name, "whoami".into()));
    create_cap_grant!(
        CapGrantEntry {
            tag: "".into(),
            // empty access converts to unrestricted
            access: ().into(),
            functions,
        }
    )?;

    Ok(())
}

// returns the current agent info
#[hdk_extern]
fn whoami(_: ()) -> ExternResult<AgentInfo> {
    Ok(agent_info!()?)
}

// returns the agent info reported by the given pub key
// in theory the output is the same as the input
// it's just that the output comes _from the opinion of the remote agent_
#[hdk_extern]
fn whoarethey(agent_pubkey: AgentPubKey) -> ExternResult<AgentInfo> {
    let response: ZomeCallInvocationResponse = call_remote!(
        agent_pubkey,
        zome_info!()?.zome_name,
        "whoami".to_string().into(),
        ().into(),
        ().try_into()?
    )?;

    match response {
        ZomeCallInvocationResponse::ZomeApiFn(guest_output) => Ok(guest_output.into_inner().try_into()?),
        // we're just panicking here because our simple tests can always call set_access before
        // calling whoami, but in a real app you'd want to handle this by returning an `Ok` with
        // something meaningful to the extern's client
        ZomeCallInvocationResponse::Unauthorized => unreachable!(),
    }
}
