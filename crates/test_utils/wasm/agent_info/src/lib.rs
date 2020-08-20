use hdk3::prelude::*;

#[hdk(extern)]
fn agent_info(_: ()) -> ExternResult<AgentInfo> {
    Ok(agent_info!()?)
}
