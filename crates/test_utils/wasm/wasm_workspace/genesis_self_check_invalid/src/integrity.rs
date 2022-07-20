use hdi::prelude::*;

#[hdk_extern]
fn genesis_self_check(_: GenesisSelfCheckData) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Invalid("esoteric edge case".into()))
}
