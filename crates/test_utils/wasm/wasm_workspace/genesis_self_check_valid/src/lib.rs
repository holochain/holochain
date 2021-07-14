use hdk::prelude::*;

#[hdk_extern]
fn genesis_self_check(data: GenesisSelfCheckData) -> ExternResult<ValidateCallbackResult> {
    let GenesisSelfCheckData {
        dna_def: _,
        membrane_proof: _,
        agent_key: _,
    } = data;
    Ok(ValidateCallbackResult::Valid)
}
