use hdk::prelude::*;

#[hdk_extern]
fn init(_: ()) -> ExternResult<InitCallbackResult> {
    Ok(InitCallbackResult::Fail("because i said so".to_string()))
}
