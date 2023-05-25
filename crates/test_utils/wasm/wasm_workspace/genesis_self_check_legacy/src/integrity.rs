use hdi::prelude::*;

// Written in full to avoid the renaming internal to the macro.
#[no_mangle]
pub extern "C" fn genesis_self_check(guest_ptr: usize, len: usize) -> DoubleUSize {
    map_extern_preamble!(guest_ptr, len, inner, GenesisSelfCheckDataV1, ExternResult<ValidateCallbackResult>);
    match genesis_self_check_legacy(inner) {
        Ok(v) => map_extern::encode_to_guestptrlen(v),
        Err(e) => return_err_ptr(e),
    }
}

fn genesis_self_check_legacy(data: GenesisSelfCheckDataV1) -> ExternResult<ValidateCallbackResult> {
    let GenesisSelfCheckDataV1 {
        dna_info: _dna_info,
        membrane_proof: _membrane_proof,
        agent_key: _agent_key,
    } = data;
    Ok(ValidateCallbackResult::Valid)
}
