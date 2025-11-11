use crate::prelude::*;

/// Locks the local chain to commence a countersigning session.
///
/// Every participant, including the initiator MUST call this function. The initiator keeps their
/// own `PreflightRequestAcceptance` and all other participants MUST send their
/// `PreflightRequestAcceptance` back to the initiator. The session initiator is then responsible
/// for building the corresponding countersigning entry for everyone to sign.
///
/// It is left to the app developer to choose how to coordinate sharing the [PreflightRequest]s and
/// collecting the [PreflightRequestAcceptance]s. Concurrent remote calls are probably the simplest
/// mechanism to distribute and accept preflight requests before the session times out.
///
/// Note that once this function has been called, each participant's chain is locked until the
/// countersigning is complete. This call does not block, so it up to the app developer to avoid
/// making any other calls that would write to the chain until the countersigning is complete.
/// There are multiple cases that you should consider handling:
///   - If the countersigning completes successfully then Holochain will emit a system signal. You
///     can listen for a system signal of type `SuccessfulCountersigning`. This is sent after your
///     chain has been unlocked, so it is safe to write to the chain again.
///   - If you take an invalid action during the countersigning process, such as trying to write an
///     entry that does not match the current countersigning session, then the session will fail and
///     your chain will be immediately unlocked. In this case, you may attempt to start a new
///     session, but you should be aware that other participants will still have their chains
///     locked. See the next bullet point.
///   - If a participant fails to complete the countersigning process, then the session will run
///     until the [CounterSigningSessionTimes] expire. Your safest option, if you do not get a
///     successful completion signal, is to wait for the session to expire. Your chain will
///     automatically be unlocked at that point, and you can resume writing to the source chain or
///     start a new countersigning session. You should be aware of the docs for [session_times_from_millis]
///     which mention that other participants may end their session at a slightly different time.
///     This means you cannot be certain that the same peers will be able to participate in a new
///     session exactly at the time when you are able to start one.
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
