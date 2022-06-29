use holochain_deterministic_integrity::prelude::*;

#[hdk_extern]
fn validate(_: Op) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}
