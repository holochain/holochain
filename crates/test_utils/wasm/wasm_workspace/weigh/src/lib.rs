use hdk::prelude::*;

#[hdk_extern]
fn weigh(input: WeighInput) -> ExternResult<WeighCallbackResult> {
    Ok(WeighCallbackResult {
        rate_bucket: 3,
        rate_weight: match input {
            WeighInput::Link(_) => 1,
            WeighInput::Entry(_) => 2,
        },
    })
}
