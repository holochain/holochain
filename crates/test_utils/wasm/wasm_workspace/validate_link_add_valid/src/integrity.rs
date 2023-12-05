use hdi::prelude::*;

#[hdk_extern]
pub fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op {
        // This is a pretty pointless example as everything is valid.
        Op::RegisterCreateLink(RegisterCreateLink {  ..  }) => Ok(ValidateCallbackResult::Valid),
        _ => Ok(ValidateCallbackResult::Valid),
    }
}
