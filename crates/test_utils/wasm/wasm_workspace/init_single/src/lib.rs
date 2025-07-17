use crate::integrity::LinkTypes;
use hdk::prelude::*;

mod integrity;

#[hdk_extern]
fn init() -> ExternResult<InitCallbackResult> {
    let my_agent_info = agent_info()?;

    // Look for links from our agent key, locally
    let links = get_links(
        LinkQuery::try_new(my_agent_info.agent_initial_pubkey.clone(), LinkTypes::Once)?,
        GetStrategy::Local,
    )?;

    // If any of those links were authored by us, then init has already run and we're going to fail
    if links
        .iter()
        .any(|link| link.author == my_agent_info.agent_initial_pubkey)
    {
        return Ok(InitCallbackResult::Fail("Already initialized".to_string()));
    }

    // Otherwise, this is the first init and we can create the link
    create_link(
        my_agent_info.agent_initial_pubkey,
        my_agent_info.chain_head.0,
        LinkTypes::Once,
        (),
    )?;

    Ok(InitCallbackResult::Pass)
}
