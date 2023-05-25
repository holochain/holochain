use hdi::prelude::*;

#[hdk_extern]
fn genesis_self_check_1(data: GenesisSelfCheckDataV1) -> ExternResult<ValidateCallbackResult> {
    let GenesisSelfCheckDataV1 {
        dna_info,
        membrane_proof,
        agent_key,
    } = data;
    Ok(ValidateCallbackResult::Valid)
}
