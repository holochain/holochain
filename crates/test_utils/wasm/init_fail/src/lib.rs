use hdk3::prelude::*;

#[hdk(extern)]
fn init(_: ()) -> ExternResult<InitCallbackResult> {
    Ok(InitCallbackResult::Fail("because i said so".to_string()))
}
