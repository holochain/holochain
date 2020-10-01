use hdk3::prelude::*;

#[hdk_extern]
fn validate(_: Element) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}
