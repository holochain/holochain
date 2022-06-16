use hdk::prelude::*;

#[hdk_extern]
fn rate_limits(_: ()) -> ExternResult<RateLimitsCallbackResult> {
    Ok(vec![RateLimit {
        capacity: 1000,
        drain_amount: 100,
        drain_interval_ms: 10,
    }]
    .into())
}
