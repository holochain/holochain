use hdk::prelude::*;

#[hdk_extern]
fn get_links_from_other_zome(agent_pubkey: AgentPubKey) -> ExternResult<Links> {
    Ok(call_remote(
        agent_pubkey,
        "call_remote_callee".to_string().into(),
        "get_links_on_foo".to_string().into(),
        None,
        &(),
    )?)
}

#[hdk_extern]
fn get_links_from_my_other_zome(_: ()) -> ExternResult<Links> {
    let agent_pubkey = agent_info()?.agent_initial_pubkey;
    Ok(call_remote(
        agent_pubkey,
        "call_remote_callee".to_string().into(),
        "get_links_on_foo".to_string().into(),
        None,
        &(),
    )?)
}
