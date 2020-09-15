use hdk3::prelude::*;

#[hdk_extern]
pub fn validate_link(_: ValidateCreateLinkData) -> ExternResult<ValidateCreateLinkCallbackResult> {
    Ok(ValidateCreateLinkCallbackResult::Invalid(
        "esoteric edge case (link version)".into(),
    ))
}
