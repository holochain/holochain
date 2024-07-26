#![allow(missing_docs)]
use std::time::Duration;

#[macro_export]
macro_rules! wait_for {
    ($wait:expr, $test:expr, $check:expr, $assert:expr) => {{
        let mut w = $wait;
        let assert = $assert;
        loop {
            let o = $test;
            let check = $check;
            if !w.wait_any().await || check(&o) {
                assert(o);
                break;
            }
        }
    }};
}

#[macro_export]
macro_rules! wait_for_10s {
    ($test:expr, $check:expr, $assert:expr) => {{
        let wait_for = $crate::test_utils::WaitFor::ten_s();
        $crate::wait_for!(wait_for, $test, $check, $assert)
    }};
}

#[macro_export]
macro_rules! wait_for_1m {
    ($test:expr, $check:expr, $assert:expr) => {
        let wait_for = $crate::test_utils::WaitFor::one_m();
        $crate::wait_for!(wait_for, $test, $check, $assert)
    };
}

#[macro_export]
macro_rules! assert_retry {
    ($wait:expr, $test:expr $(, $reason:literal)?) => {
        $crate::wait_for!($wait, $test, |x: &bool| *x, |x: bool| assert!(x $(, $reason)?))
    };
}

#[macro_export]
macro_rules! assert_eq_retry {
    ($wait:expr, $test:expr, $check:expr $(, $reason:literal)?) => {
        $crate::wait_for!($wait, $test, |x| x == &$check, |x| assert_eq!(x, $check $(, $reason)?))
    };
}

#[macro_export]
macro_rules! assert_retry_10s {
    ($test:expr $(, $reason:literal)? $(,)?) => {
        let wait_for = $crate::test_utils::WaitFor::ten_s();
        $crate::assert_retry!(wait_for, $test  $(, $reason:literal)?)
    };
}

#[macro_export]
macro_rules! assert_eq_retry_10s {
    ($test:expr, $check:expr $(, $reason:literal)? $(,)?) => {
        let wait_for = $crate::test_utils::WaitFor::ten_s();
        $crate::assert_eq_retry!(wait_for, $test, $check  $(, $reason:literal)?)
    };
}

#[macro_export]
macro_rules! assert_retry_1m {
    ($test:expr $(, $reason:literal)? $(,)?) => {
        let wait_for = $crate::test_utils::WaitFor::one_m();
        $crate::assert_retry!(wait_for, $test $(, $reason:literal)?)
    };
}

#[macro_export]
macro_rules! assert_eq_retry_1m {
    ($test:expr, $check:expr $(, $reason:literal)? $(,)?) => {
        let wait_for = $crate::test_utils::WaitFor::one_m();
        $crate::assert_eq_retry!(wait_for, $test, $check  $(, $reason:literal)?)
    };
}

#[macro_export]
macro_rules! assert_eq_retry_5m {
    ($test:expr, $check:expr $(, $reason:literal)? $(,)?) => {
        let wait_for = $crate::test_utils::WaitFor::five_m();
        $crate::assert_eq_retry!(wait_for, $test, $check  $(, $reason:literal)?)
    };
}

#[derive(Debug, Clone)]
/// Generic waiting for some test property to
/// be true. This allows early exit from waiting when
/// the condition becomes true but will wait up to a
/// maximum if the condition is not true.
pub struct WaitFor {
    num_attempts: u32,
    attempt: u32,
    delay: Duration,
}

impl WaitFor {
    /// Create a new wait for from a number of attempts and delay in between attempts
    pub fn new(total: Duration, num_attempts: u32) -> Self {
        Self {
            num_attempts,
            attempt: 0,
            delay: total / num_attempts,
        }
    }

    /// Wait for 1s checking every 100ms.
    pub fn one_s() -> Self {
        Self::new(std::time::Duration::from_secs(1), 10)
    }

    /// Wait for 10s checking every 100ms.
    pub fn ten_s() -> Self {
        Self::new(std::time::Duration::from_secs(10), 100)
    }

    /// Wait for 1 minute checking every 500ms.
    pub fn one_m() -> Self {
        Self::new(std::time::Duration::from_secs(60), 120)
    }

    /// Wait for 5 minutes checking every 1000ms.
    pub fn five_m() -> Self {
        Self::new(std::time::Duration::from_secs(5 * 60), 5 * 60)
    }

    /// Wait for some time before trying again.
    /// Will return false when you should stop waiting.
        pub async fn wait_any(&mut self) -> bool {
        if self.attempt >= self.num_attempts {
            return false;
        }
        self.attempt += 1;
        tracing::debug!(attempt = ?self.attempt, out_of = ?self.num_attempts, delaying_for = ?self.delay);
        tokio::time::sleep(self.delay).await;
        true
    }
}

#[tokio::test]
async fn wait_for_tests() {
    wait_for!(WaitFor::one_s(), true, |&x| x, |x: bool| assert!(x));
    wait_for!(WaitFor::one_s(), false, |&x| x, |x: bool| assert!(!x));
}
