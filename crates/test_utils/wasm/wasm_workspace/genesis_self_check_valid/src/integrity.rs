use hdi::prelude::*;

#[hdk_extern]
fn genesis_self_check(data: GenesisSelfCheckData) -> ExternResult<ValidateCallbackResult> {
    let GenesisSelfCheckDataV2 (_maybe_membrane_proof) = data;
    let _dna_info: DnaInfoV2 = dna_info()?;
    Ok(ValidateCallbackResult::Valid)
}
