use hdk3::prelude::*;

#[hdk_extern]
fn validate_agent(_: Element) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}

#[hdk_extern]
fn validate(_: ValidateData) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Invalid("esoteric edge case".into()))
}
