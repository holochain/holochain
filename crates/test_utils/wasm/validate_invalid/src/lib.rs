use hdk3::prelude::*;

#[hdk(extern)]
fn validate_agent(_: ()) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateAgentCallbackResult::Valid)
}

#[hdk(extern)]
fn validate(_: ()) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Invalid("esoteric edge case".into()))
}
