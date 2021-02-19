use hdk::prelude::*;

#[hdk_extern]
fn agent_info(_: ()) -> ExternResult<AgentInfo> {
    hdk::prelude::agent_info()
}
