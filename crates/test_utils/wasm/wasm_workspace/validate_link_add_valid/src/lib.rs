use hdk3::prelude::*;

#[hdk_extern]
pub fn validate_link(_: ValidateCreateLinkData) -> ExternResult<ValidateCreateLinkCallbackResult> {
    Ok(ValidateCreateLinkCallbackResult::Valid)
}
