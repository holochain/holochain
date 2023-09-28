use hdi::prelude::*;
#[hdk_entry_helper]
#[derive(Clone)]
pub struct File {
    pub data: SerializedBytes,
    pub uid: i64,
}
pub fn validate_create_link_all_images(
    base: AnyLinkableHash,
    target: AnyLinkableHash,
) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}
pub fn validate_delete_link_all_images(
    base: AnyLinkableHash,
    target: AnyLinkableHash,
) -> ExternResult<ValidateCallbackResult> {
    Ok(
        ValidateCallbackResult::Invalid(
            String::from("AllImages links cannot be deleted"),
        ),
    )
}
