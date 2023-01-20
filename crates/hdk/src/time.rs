use crate::prelude::*;

/// Current system time from the host.
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
/// - the host signing times into chain actions is using the same clock in the host_fn call so the
///   the sys_time returned by a function for inclusion in an entry will always be less than or
///   equal to the time in the action of that entry unless:
///     - the user manually changed the system time backwards between this host_fn being called and
///       an entry using it being committed (NTP avoids making time go backward by stretching it)
///     - the sys_time call ran on a different machine, e.g. via a call_remote, to the machine that
///       commits it into an entry
///   so your app can decide to implement validation logic that fails any sys time that comes after
///   the time signed in the action if that makes sense.
/// - the times in the actions of the local source chain must increase monotonically and all
///   actions are pushed to the neighbourhood of the agent publishing them, so the agent activity
///   authorities will immediately warrant any actions running chronologically backwards
/// - the times within a single record may be relatively secure but are easy to manipulate in an
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
/// See <https://blog.cloudflare.com/roughtime/>
///
/// @todo
/// Another option is to use proof of work style constructions to roughly throttle the speed that
/// things can be done without relying on absolute times, or even that users experience the same
/// throttling due to differences in CPU/GPU performance on the POW algorithm.
/// See <https://zkga.me/> uses this as a game mechanic
///
/// @todo
/// Other p2p type time syncing algorithms that allow peers to adjust their clock offsets to agree
/// on the current time within relatively tight accuracy/precision up-front in a relatively trusted
/// environment e.g. a chess game between friends with time moves that balances security/trust and
/// flaky networking, etc.
pub fn sys_time() -> ExternResult<Timestamp> {
    HDK.with(|h| h.borrow().sys_time(()))
}

/// Adds a function from the current zome to the scheduler.
///
/// Any schedulable function from the current zome can be added to the scheduler
/// by calling this function. Schedulable functions are infallable and MUST return
/// their next trigger time. Trigger times are expressed as either "ephemeral"
/// which means they will run as "best effort" after some duration, or "persisted"
/// which uses crontab like syntax to trigger the scheduled function many times.
/// Ephemeral scheduled functions do not outlive the running conductor but
/// persisted scheduled functions will continue to function after a reboot.
/// Persisted functions MUST continue to return the same persisted crontab every
/// time they are triggered if they wish to maintain their schedule. They MAY change
/// their schedule by returning a different crontab or even returning an ephemeral
/// trigger or `None` for no further triggers. If this is the initial trigger of
/// a scheduled function the input schedule will be `None`, otherwise it will be
/// whatever was returned by the previous invocation that triggered the current
/// invocation.
///
/// Scheduling a function will trigger it once unconditionally on the next iteration
/// of the conductor's internal scheduler loop. The frequency of the loop is subject
/// to change between conductor versions and MAY be configurable in the future, so
/// happ devs are advised NOT to assume or rely on any specific granularity. For
/// example the loop has historically ranged from once every 100ms to every 10s.
///
/// As `schedule` is callable in any coordination context it could even be called
/// as the result of inbound remote calls or many times concurrently by some client.
/// Both floods of inbound scheduling requests and "confused deputy" situations
/// must be handled by the conductor.
///
/// - Scheduling a function is idempotent. If it is already scheduled the existing
///   schedule will be respected and the `schedule` call is a noop. If the function
///   is not currently scheduled, even if it recently returned `None` from a previous
///   schedule, it will immediately be added for inclusion in the next scheduler
///   loop iteration.
/// - Scheduled functions ALWAYS run as the author of the chain they run for. Any
///   appropriate cap grants must be implemented in front of the `schedule` call
///   as the provenance of the scheduling agent is lost as soon as the original
///   zome call returns. This resolves the natural tension between disambiguating
///   and handling potentially hundreds of scheduled calls under different
///   provenances, while also wanting a single lightweight and idempotent scheduler.
/// - Scheduled functions are infallible and their only input and output is their
///   current and next schedule trigger. The `#[hdk_extern(infallible)]` attribute
///   facilitates this pattern separate to other zome externs that are both fallible
///   and support arbitrary inputs and outputs. Any errors on the host side will
///   simply be logged and otherwise ignored.
///   ```ignore
///   #[hdk_extern(infallible)]
///   fn scheduled_fn(_: Option<Schedule>) -> Option<Schedule> {}
///   ```
///   This is because the scheduler runs in a background loop and unlike regular
///   zome calls there is no client or workflow attached to report back to or
///   handle errors. There are no inputs and outputs to scheduleable functions as
///   we don't want to provide the opportunity to smuggle in data that will be run
///   by the author as themselves if the input originated from some caller who
///   merely held a cap grant to trigger the schedule.
/// - Happ devs MUST assume that malicious agents are able to trigger scheduled
///   functions at the "wrong time" and write their scheduled functions defensively
///   to noop then delay or terminate themselves if triggered during the incorrect
///   time window.
///
/// It is worth noting that at the time of writing, `init` callbacks are lazy in
/// that they do not execute until/unless some other zome call runs for the first
/// time after installation. It is possible to schedule functions during `init`
/// but happ devs should be mindful that this may not happen immediately or ever
/// after happ installation.
///
/// The only argument to `schedule` is the name of the schedulable function in the
/// current zome to be scheduled.
pub fn schedule(scheduled_fn: &str) -> ExternResult<()> {
    HDK.with(|h| h.borrow().schedule(String::from(scheduled_fn)))
}

/// @todo Not implemented
pub fn sleep(wake_after: std::time::Duration) -> ExternResult<()> {
    HDK.with(|h| h.borrow().sleep(wake_after))
}
