use hdk3::prelude::*;

#[hdk_extern]
fn init(_: ()) -> ExternResult<InitCallbackResult> {
    // grant unrestricted access to accept_cap_claim so other agents can send us claims
    let mut functions: GrantedFunctions = HashSet::new();
    functions.insert((zome_info!()?.zome_name, "accept_cap_claim".into()));
    commit_cap_grant!(
        CapGrantEntry {
            // empty access converts to unrestricted
            access: ().into(),
            functions,
            ..Default::default()
        }
    )?;

    Ok(InitCallbackResult::Pass)
}

#[macro_export]
macro_rules! set_externs_cap_unrestricted {
    ( $externs:expr ) => {{
        match $crate::prelude::zome_info!() {
            Ok(zome_info) => {
                $crate::prelude::commit_cap_grant!(
                    CapGrantEntry {
                        // a standalone secret converts to transferable access
                        access: secret.into(),
                        ..Default::default()
                    }
                )
            },
            Err(e) => Err(e),
        }
    }};
}

#[hdk_extern]
pub fn cap_secret(_: ()) -> ExternResult<CapSecret> {
    Ok(generate_cap_secret!()?)
}

#[hdk_extern]
pub fn transferable_cap_grant(secret: CapSecret) -> ExternResult<HeaderHash> {
    Ok(commit_cap_grant!(
        CapGrantEntry {
            tag: "".into(),
            access: secret.into(),
            functions: HashSet::new(),
        }
    )?)
}

#[hdk_extern]
fn get_entry(header_hash: HeaderHash) -> ExternResult<GetOutput> {
    Ok(GetOutput::new(get!(header_hash)?))
}

/// accepts a cap claim and commits it to the local chain
/// intended to
#[hdk_extern]
fn accept_cap_claim(claim: CapClaim) -> ExternResult<HeaderHash> {
    Ok(commit_cap_claim!(claim)?)
}

#[hdk_extern]
fn needs_cap_claim(_: ()) -> ExternResult<()> {
    Ok(())
}

#[hdk_extern]
fn send_assigned_cap_claim(agent: AgentPubKey) -> ExternResult<()> {
    let tag = String::from("has_cap_claim");

    // make a new secret
    let secret = generate_cap_secret!()?;

    // grant the secret as assigned (can only be used by the intended agent)
    let mut functions: GrantedFunctions = HashSet::new();
    let this_zome = zome_info!()?.zome_name;
    functions.insert((this_zome.clone(), "needs_cap_claim".into()));
    commit_cap_grant!(CapGrantEntry {
        access: (secret, agent.clone()).into(),
        functions,
        tag: tag.clone(),
        ..Default::default()
    })?;

    // send the assigned cap token
    call_remote!(
        agent,
        this_zome,
        "accept_cap_claim".into(),
        ().into(),
        CapClaim::new(
            tag,
            agent_info!()?.agent_latest_pubkey,
            secret,
        ).try_into()?
    )?;
    Ok(())
}
