use hdk::prelude::*;

#[hdk_extern]
fn validation_package(_: AppEntryType) -> ExternResult<ValidationPackageCallbackResult> {
    Ok(ValidationPackageCallbackResult::Fail("bad package".into()))
}
