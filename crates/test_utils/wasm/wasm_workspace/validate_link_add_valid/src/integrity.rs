use holochain_deterministic_integrity::prelude::*;

#[hdk_extern]
pub fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op {
        // This is a pretty pointless example as everything is valid.
        Op::RegisterCreateLink { .. } => Ok(ValidateCallbackResult::Valid),
        _ => Ok(ValidateCallbackResult::Valid),
    }
}