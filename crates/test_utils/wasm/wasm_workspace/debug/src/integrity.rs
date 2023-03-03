use hdi::prelude::*;

#[hdk_extern]
fn validate(_op: Op) -> ExternResult<ValidateCallbackResult> {
    tracing::info!("tracing in validation works");
    Ok(ValidateCallbackResult::Valid)
}
