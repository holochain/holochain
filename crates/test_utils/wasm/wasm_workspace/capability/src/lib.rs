use hdk3::prelude::*;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct CapFor(CapSecret, AgentPubKey);

#[hdk_extern]
fn init(_: ()) -> ExternResult<InitCallbackResult> {
    // grant unrestricted access to accept_cap_claim so other agents can send us claims
    let mut functions: GrantedFunctions = BTreeSet::new();
    functions.insert((zome_info()?.zome_name, "accept_cap_claim".into()));
    // functions.insert((zome_info()?.zome_name, "needs_cap_claim".into()));
    create_cap_grant(CapGrantEntry {
        tag: "".into(),
        // empty access converts to unrestricted
        access: ().into(),
        functions,
    })?;

    Ok(InitCallbackResult::Pass)
}

#[hdk_extern]
pub fn cap_secret(_: ()) -> ExternResult<CapSecret> {
    CapSecret::try_from_random()
}

fn cap_grant_entry(secret: CapSecret) -> ExternResult<CapGrantEntry> {
    let mut functions: GrantedFunctions = BTreeSet::new();
    let this_zome = zome_info()?.zome_name;
    functions.insert((this_zome, "needs_cap_claim".into()));
    Ok(CapGrantEntry {
        tag: "".into(),
        access: secret.into(),
        functions,
    })
}

#[hdk_extern]
pub fn transferable_cap_grant(secret: CapSecret) -> ExternResult<HeaderHash> {
    create_cap_grant(cap_grant_entry(secret)?)
}

#[hdk_extern]
pub fn roll_cap_grant(header_hash: HeaderHash) -> ExternResult<HeaderHash> {
    let secret = CapSecret::try_from_random()?;
    update_cap_grant(header_hash, cap_grant_entry(secret)?)
}

#[hdk_extern]
pub fn delete_cap_grant(header_hash: HeaderHash) -> ExternResult<HeaderHash> {
    hdk3::prelude::delete_cap_grant(header_hash)
}

#[hdk_extern]
fn get_entry(header_hash: HeaderHash) -> ExternResult<Option<Element>> {
    get(header_hash, GetOptions::latest())
}

/// accepts a cap claim and commits it to the local chain
/// intended to
#[hdk_extern]
fn accept_cap_claim(claim: CapClaim) -> ExternResult<HeaderHash> {
    create_cap_claim(claim)
}

#[hdk_extern]
fn needs_cap_claim(_: ()) -> ExternResult<()> {
    Ok(())
}

#[hdk_extern]
fn try_cap_claim(cap_for: crate::CapFor) -> ExternResult<ZomeCallResponse> {
    Ok(call_remote(
        cap_for.1,
        zome_info()?.zome_name,
        "needs_cap_claim".to_string().into(),
        Some(cap_for.0),
        &(),
    )?)
}

#[hdk_extern]
fn send_assigned_cap_claim(agent: AgentPubKey) -> ExternResult<()> {
    let tag = String::from("has_cap_claim");

    // make a new secret
    let secret = CapSecret::try_from_random()?;

    // grant the secret as assigned (can only be used by the intended agent)
    let mut functions: GrantedFunctions = BTreeSet::new();
    let this_zome = zome_info()?.zome_name;
    functions.insert((this_zome.clone(), "needs_cap_claim".into()));
    create_cap_grant(CapGrantEntry {
        access: (secret, agent.clone()).into(),
        functions,
        tag: tag.clone(),
    })?;

    // send the assigned cap token
    call_remote(
        agent,
        this_zome,
        "accept_cap_claim".into(),
        None,
        &CapClaim::new(tag, agent_info()?.agent_latest_pubkey, secret),
    )?;
    Ok(())
}
