use hdk3::prelude::*;

#[hdk(extern)]
fn validation_package(_: AppEntryType) -> ExternResult<ValidationPackageCallbackResult> {
    Ok(ValidationPackageCallbackResult::Fail("bad package".into()))
}
