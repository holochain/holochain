use crate::prelude::*;

pub fn accept_countersigning_preflight_request(
    preflight_request: PreflightRequest,
) -> ExternResult<PreflightRequestAcceptance> {
    // Host should:
    // - Check system constraints on request
    // - Freeze chain for session end
    // - Build response
    HDK.with(|h| {
        h.borrow()
            .accept_countersigning_preflight_request(preflight_request)
    })
}

pub fn session_times_from_millis(ms: u64) -> ExternResult<CounterSigningSessionTimes> {
    let start = sys_time()?;
    let end = start + core::time::Duration::from_millis(ms);
    Ok(CounterSigningSessionTimes::new(
        start,
        end.map_err(|e| WasmError::Guest(e.to_string()))?,
    ))
}
