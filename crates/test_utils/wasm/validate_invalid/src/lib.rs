use hdk3::prelude::*;

#[hdk(extern)]
fn validate_agent(_: Entry) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}

#[hdk(extern)]
fn validate(_: Entry) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Invalid("esoteric edge case".into()))
}
