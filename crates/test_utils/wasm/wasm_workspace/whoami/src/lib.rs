use hdk::prelude::*;

enum Zomes {
    CreateEntry,
}

impl From<Zomes> for ZomeName {
    fn from(z: Zomes) -> Self {
        match z {
            Zomes::CreateEntry => ZomeName("create_entry".into()),
        }
    }
}

#[hdk_extern]
fn set_access(_: ()) -> ExternResult<()> {
    let mut functions: GrantedFunctions = BTreeSet::new();
    functions.insert((zome_info()?.name, "whoami".into()));
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
    agent_info()
}

// returns the agent info reported by the given pub key
// in theory the output is the same as the input
// it's just that the output comes _from the opinion of the remote agent_
#[hdk_extern]
fn whoarethey(agent_pubkey: AgentPubKey) -> ExternResult<AgentInfo> {
    let zome_call_response: ZomeCallResponse = call_remote(
        agent_pubkey,
        zome_info()?.name,
        "whoami".to_string().into(),
        None,
        &(),
    )?;
    match zome_call_response {
        // The decode() type needs to match the return type of "whoami"
        ZomeCallResponse::Ok(v) => Ok(v.decode().map_err(|e| wasm_error!(e))?),
        // This should be handled in real code.
        _ => unreachable!(),
    }
}

// returns the agent info reported by the given pub key
// in theory the output is the same as the input
// it's just that the output comes _from the opinion of the remote agent_
#[hdk_extern]
fn who_are_they_local(cell_id: CellId) -> ExternResult<AgentInfo> {
    let zome_call_response: ZomeCallResponse = call(
        CallTargetCell::OtherCell(cell_id),
        zome_info()?.name,
        "whoami".to_string().into(),
        None,
        &(),
    )?;
    match zome_call_response {
        ZomeCallResponse::Ok(v) => Ok(v.decode().map_err(|e| wasm_error!(e))?),
        // This should be handled in real code.
        _ => unreachable!(),
    }
}

#[hdk_extern]
fn who_are_they_role(role_name: RoleName) -> ExternResult<AgentInfo> {
    let zome_call_response: ZomeCallResponse = call(
        CallTargetCell::OtherRole(role_name),
        zome_info()?.name,
        "whoami".to_string().into(),
        None,
        &(),
    )?;
    match zome_call_response {
        ZomeCallResponse::Ok(v) => Ok(v.decode().map_err(|e| wasm_error!(e))?),
        // This should be handled in real code.
        _ => unreachable!(),
    }
}

/// Call the create entry zome from this zome.
/// The cell id must point to a cell which includes
/// the "create_entry" zome.
#[hdk_extern]
fn call_create_entry(cell_id: CellId) -> ExternResult<ActionHash> {
    let zome_call_response: ZomeCallResponse = call(
        CallTargetCell::OtherCell(cell_id),
        Zomes::CreateEntry,
        "create_entry".to_string().into(),
        None,
        &(),
    )?;
    match zome_call_response {
        ZomeCallResponse::Ok(v) => Ok(v.decode().map_err(|e| wasm_error!(e))?),
        // This should be handled in real code.
        _ => unreachable!(),
    }
}
