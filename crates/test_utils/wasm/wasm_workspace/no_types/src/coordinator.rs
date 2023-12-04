use hdk::prelude::*;

#[hdk_extern]
fn init(_: ()) -> ExternResult<InitCallbackResult> {
    Ok(InitCallbackResult::Pass)
}
