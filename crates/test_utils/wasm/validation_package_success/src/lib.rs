use hdk3::prelude::*;

#[hdk(extern)]
fn validation_package(_: ()) -> ExternResult<ValidationPackageCallbackResult> {
    Ok(ValidationPackageCallbackResult::Success(ValidationPackage))
}
