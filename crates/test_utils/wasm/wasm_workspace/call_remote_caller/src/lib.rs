use hdk3::prelude::*;

#[hdk_extern]
fn get_links_from_other_zome(_: ()) -> ExternResult<Links> {
    let agent_pubkey = agent_info!()?.agent_latest_pubkey;
    let response: ZomeCallResponse = call_remote!(
        agent_pubkey,
        "call_remote_callee".to_string().into(),
        "get_links_on_foo".to_string().into(),
        None,
        ().try_into()?
    )?;

    match response {
        ZomeCallResponse::Ok(guest_output) => Ok(guest_output.into_inner().try_into()?),
        // we're just panicking here because our simple tests sets access in init before
        // calling whoami, but in a real app you'd want to handle this by returning an `Ok` with
        // something meaningful to the extern's client
        ZomeCallResponse::Unauthorized => unreachable!("Should have access"),
    }
}
