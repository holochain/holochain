use hdk::prelude::*;

#[hdk_extern]
pub fn validate_create_link(_: ValidateCreateLinkData) -> ExternResult<ValidateLinkCallbackResult> {
    Ok(ValidateLinkCallbackResult::Invalid(
        "esoteric edge case (link version)".into(),
    ))
}
