use crate::prelude::*;

/// Trivial wrapper to return the current system time from the host.
///
/// System time doesn't accept any arguments so usage is as simple as:
///
/// ```ignore
/// let now = sys_time()?;
/// ```
///
/// Note: sys_time returns a result like all host fns so `?` or `.ok()` is needed.
///
/// System times can be considered "secure" or "insecure" situationally, some things to consider:
///
/// - the host signing times into chain headers is using the same clock in the host_fn call so the
///   the sys_time returned by a function for inclusion in an entry will always be less than or
///   equal to the time in the header of that entry unless:
///     - the user manually changed the system time backwards between this host_fn being called and
///       an entry using it being committed (NTP avoids making time go backward by stretching it)
///     - the sys_time call ran on a different machine, e.g. via a call_remote, to the machine that
///       commits it into an entry
///   so your app can decide to implement validation logic that fails any sys time that comes after
///   the time signed in the header if that makes sense.
/// - the times in the headers of the local source chain must increase monotonically and all
///   headers are pushed to the neighbourhood of the agent publishing them, so the agent activity
///   authorities will immediately warrant any headers running chronologically backwards
/// - the times within a single element may be relatively secure but are easy to manipulate in an
///   absolute sense, the user can simply change their system clock before attempting a commit to
///   any time that is equal to or later than their current chain head
///
/// This is an improvement on pushing time collection back onto the "client" which can't
/// guarantee that it is seeing the same time as the rust host, so this enables stricter validation
/// logic.
///
/// @todo
/// Sys times aren't the final word on secure times, for another option it may be best to use the
/// roughtime protocol which redundantly fetches cryptographically signed times from multiple
/// different servers and cross-references them.
/// There is a POC for this but it's not in core yet (requires at least UDP calls from the host).
/// Note that the **redundant fetching** and **cross-referencing** parts are critical, even though
/// they add a lot of complexity to the protocol. Failure to do this brought down the ETH2.0
/// Medalla testnet due to a single roughtime server from cloudflare being 24 hours off.
/// Note also that roughtime, or any other "secure timestamping" option requires the agent to be
/// online at the time of generating times, which runs counter to the requirement that holochain
/// support "agent centric, offline first" behaviour, but may be acceptable or even a neccessary
/// evil for specific application logic.
/// The other challenge with roughtime is list management to track the list of valid servers over
/// time, which might rely on agents providing snapshots of links to public keys (i.e. representing
/// the roughtime ecosystem itself in a happ).
///
/// @see https://blog.cloudflare.com/roughtime/
///
/// @todo
/// Another option is to use proof of work style constructions to roughly throttle the speed that
/// things can be done without relying on absolute times, or even that users experience the same
/// throttling due to differences in CPU/GPU performance on the POW algorithm.
/// @see https://zkga.me/ uses this as a game mechanic
///
/// @todo
/// Other p2p type time syncing algorithms that allow peers to adjust their clock offsets to agree
/// on the current time within relatively tight accuracy/precision up-front in a relatively trusted
/// environment e.g. a chess game between friends with time moves that balances security/trust and
/// flaky networking, etc.
pub fn sys_time() -> HdkResult<core::time::Duration> {
    Ok(host_call::<SysTimeInput, SysTimeOutput>(__sys_time, &SysTimeInput::new(()))?.into_inner())
}
