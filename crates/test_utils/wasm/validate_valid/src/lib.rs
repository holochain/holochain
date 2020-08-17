use hdk3::prelude::*;

#[hdk(extern)]
fn validate(_: ()) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}
