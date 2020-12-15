use hdk3::prelude::*;

#[hdk_extern]
fn validate(_: ValidateData) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}
