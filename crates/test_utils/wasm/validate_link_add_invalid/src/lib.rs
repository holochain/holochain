use hdk3::prelude::*;

#[hdk(extern)]
pub fn validate_link(_: ValidateLinkAddData) -> ExternResult<ValidateLinkAddCallbackResult> {
    Ok(ValidateLinkAddCallbackResult::Invalid(
        "esoteric edge case (link version)".into(),
    ))
}
