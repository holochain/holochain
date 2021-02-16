use hdk3::prelude::*;

#[hdk_extern]
fn agent_info(_: ()) -> ExternResult<AgentInfo> {
    hdk3::prelude::agent_info()
}
