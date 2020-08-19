use hdk3::prelude::*;

#[hdk(extern)]
fn validate(_: Entry) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}
