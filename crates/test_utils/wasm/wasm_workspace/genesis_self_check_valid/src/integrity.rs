use hdi::prelude::*;

#[hdk_extern]
fn genesis_self_check(data: GenesisSelfCheckData) -> ExternResult<ValidateCallbackResult> {
    let GenesisSelfCheckDataV2 {
        membrane_proof: _maybe_membrane_proof,
        agent_key: _agent_key,
    } = data;
    let _dna_info: DnaInfoV2 = dna_info()?;
    Ok(ValidateCallbackResult::Valid)
}
