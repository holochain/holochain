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
