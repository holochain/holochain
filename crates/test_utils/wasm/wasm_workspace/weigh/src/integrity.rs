use hdk::prelude::*;

#[hdk_extern]
fn weigh(input: WeighInput) -> ExternResult<WeighCallbackResult> {
    Ok(WeighCallbackResult {
        bucket_id: 3,
        units: match input {
            WeighInput::Link(_) => 1,
            WeighInput::Create(_, _) => 2,
            _ => 33,
        },
    })
}
