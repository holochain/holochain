use hdk3::prelude::*;

#[hdk_extern]
fn validate(_: Entry) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}
