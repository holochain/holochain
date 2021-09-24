use hdk::prelude::*;

const PING_LIMIT: usize = 5;

#[hdk_entry(id = "ping")]
struct Ping(AgentPubKey);

#[hdk_extern]
fn set_access(_: ()) -> ExternResult<()> {
    let mut functions: GrantedFunctions = BTreeSet::new();
    functions.insert((zome_info()?.zome_name, "ping".into()));
    create_cap_grant(CapGrantEntry {
        tag: "".into(),
        // empty access converts to unrestricted
        access: ().into(),
        functions,
    })?;

    Ok(())
}

#[hdk_extern]
fn ping(agent: AgentPubKey) -> ExternResult<()> {
    create_entry(Ping(agent))
}

#[hdk_extern]
fn post_commit(header_hashes: Vec<SignedHeaderHashed>) -> ExternResult<PostCommitCallbackResult> {
    let ping: Ping = must_get_entry(header_hashes.next().unwrap())?.try_into()?;
    call_remote(
        ping.0,
        zome_info()?.zome_name,
        "ping".to_string().into(),
        None,
        &agent_info().agent_pub_key,
    )?;
}
