use hdk3::prelude::*;

#[hdk_extern]
fn validation_package(_: AppEntryType) -> ExternResult<ValidationPackageCallbackResult> {
    Ok(ValidationPackageCallbackResult::Success(ValidationPackage::new(vec![])))
}
