use crate::prelude::*;

/// Locks the local chain to commence a countersigning session.
///
/// The `PreflightRequestAcceptance` MUST be sent back to the session initiator
/// so that the corresponding entry can be built for everyone to sign.
/// This function MUST be called by every signer in the signing session.
/// It doesn't matter how, although concurrent remote calls are probably the
/// simplest mechanism to distribute and accept preflight requests before the
/// session times out.
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

/// Wrapper function around `sys_time` to build `CounterSigningSessionTimes`.
/// These session times are included in the `PreflightRequest` and bound the
/// countersigning session temporally.
/// This function starts the session "now" in the opinion of the session
/// initiator and ends it after `ms` milliseconds relative to "now".
/// The countersigning parties will check these times as part of accepting the
/// preflight request so all system clocks need to be roughly aligned and the
/// ambient network latency must fit comfortably within the session duration.
pub fn session_times_from_millis(ms: u64) -> ExternResult<CounterSigningSessionTimes> {
    let start = sys_time()?;
    let end = start + core::time::Duration::from_millis(ms);
    CounterSigningSessionTimes::try_new(
        start,
        end.map_err(|e| wasm_error!(WasmErrorInner::Guest(e.to_string())))?,
    )
    .map_err(|e| wasm_error!(WasmErrorInner::Guest(e.to_string())))
}
