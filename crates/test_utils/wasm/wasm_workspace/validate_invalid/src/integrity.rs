use holochain_deterministic_integrity::prelude::*;

#[hdk_extern]
fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op {
        Op::StoreEntry {
            entry: Entry::Agent(_),
            ..
        } => Ok(ValidateCallbackResult::Valid),
        _ => Ok(ValidateCallbackResult::Invalid("esoteric edge case".into())),
    }
}
