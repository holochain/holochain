use hdk::prelude::*;

#[hdk_extern]
fn init(_: usize) -> ExternResult<InitCallbackResult> {
    Ok(InitCallbackResult::Pass)
}
