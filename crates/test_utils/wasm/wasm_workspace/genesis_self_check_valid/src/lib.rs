use hdk::prelude::*;

#[hdk_extern]
fn genesis_self_check(_: GenesisSelfCheckData) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}
