use hdk3::prelude::*;

#[hdk(extern)]
fn migrate_agent(_: ()) -> ExternResult<MigrateAgentCallbackResult> {
    Ok(MigrateAgentCallbackResult::Pass)
}
