use hdk3::prelude::*;

#[hdk(extern)]
fn migrate_agent(_: MigrateAgent) -> ExternResult<MigrateAgentCallbackResult> {
    Ok(MigrateAgentCallbackResult::Pass)
}
