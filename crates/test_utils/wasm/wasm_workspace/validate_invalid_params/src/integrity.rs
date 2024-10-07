use hdk::prelude::*;

#[hdk_extern]
fn validate(_: usize) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}
