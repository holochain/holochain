use crate::prelude::*;

pub fn accept_countersigning_preflight_request(
    _preflight_request: &PreflightRequest,
) -> ExternResult<PreflightResponse> {
    // Host should:
    // - Check system constraints on request
    // - Freeze chain for session end
    // - Build response
    todo!();
}
