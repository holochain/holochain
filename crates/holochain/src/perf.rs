/// calling into wasm with only one wasm instance involved
/// e.g. no internal callbacks or additional wasm instances in called host functions
/// typically takes 1ms or less
pub const ONE_WASM_CALL: i128 = 5_000_000;
/// callint into wasm with multiple wasm instances involved
/// e.g. calling a wasm call that then triggers a callback with its own wasm instance
/// typically wasm calls scale linearly as long as they are simple as the wasmer call overhead is
/// much larger than simple internal logic like validation etc.
pub const MULTI_WASM_CALL: i128 = 7_000_000;
/// building a wasm instance, given a wasm module
/// this is quite fast, indicative times are about 40_000 nanos
/// on circle this can be much slower at several 100k
pub const WASM_INSTANCE: i128 = 400_000;
/// geting a wasm module from the cache should be very fast
/// if you're blowing this up in a test, make sure to warm the zome cache!
pub const WASM_MODULE_CACHE_HIT: i128 = 50_000;

#[macro_export]
/// during tests, collect the start time for any timeout
/// set the return of this macro to an ident that can be passed to end_hard_timeout! below
macro_rules! start_hard_timeout {
    () => {{
        if cfg!(test) {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("Time went backwards")
        } else {
            std::time::Duration::new(0, 0)
        }
    }};
}

#[macro_export]
/// during tests, debug a timeout from a start_hard_timeout!
/// if the timeout exceeds the hard limit, panic
/// given that this panics:
/// - it only runs during tests
/// - it should only be used in tests that are run serially with #[serial_test::serial]
/// - it should use timeouts with plenty of headroom (e.g. 2.5x or more) vs. our expected times
/// - it should only be used on CRITICAL performance paths
/// - every critical perf path should be represented as a clear constant in this file
/// the goal here is NOT to enforce very tight timings but to flag perf regressions that are an
/// OOM or more above our expectations, as these can easily creep in and are hard to remove after
/// the fact on critical performance paths, after potentially months of work has piled on top of
/// a slow implementation
macro_rules! end_hard_timeout {
    ( $t0:ident, $timeout:expr ) => {{
        use std::convert::TryFrom;
        if cfg!(test) {
            let hard_timeout_nanos = i128::try_from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("Time went backwards")
                    .as_nanos(),
            )
            .unwrap()
                - i128::try_from($t0.as_nanos()).unwrap();

            let timeout_check = format!(
                "{}: {} <= {}?",
                stringify!($t0),
                hard_timeout_nanos,
                $timeout
            );
            dbg!(timeout_check);

            if hard_timeout_nanos > $timeout {
                panic!(format!(
                    "Exceeded hard timeout! {} > {} ({})",
                    hard_timeout_nanos,
                    $timeout,
                    stringify!($t0, $timeout)
                ));
            }
        }
    }};
}
