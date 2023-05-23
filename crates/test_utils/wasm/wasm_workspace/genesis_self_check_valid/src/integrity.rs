use hdi::prelude::*;

#[hdk_extern]
fn genesis_self_check(data: GenesisSelfCheckData) -> ExternResult<ValidateCallbackResult> {
    let GenesisSelfCheckDataV2 (_maybe_membrane_proof) = data;
    Ok(ValidateCallbackResult::Valid)
}
