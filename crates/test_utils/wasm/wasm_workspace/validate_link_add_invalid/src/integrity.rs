use holochain_deterministic_integrity::prelude::*;

#[hdk_extern]
pub fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op {
        Op::RegisterCreateLink { .. } => Ok(ValidateCallbackResult::Invalid(
            "esoteric edge case (link version)".into(),
        )),
        _ => Ok(ValidateCallbackResult::Valid),
    }
}
