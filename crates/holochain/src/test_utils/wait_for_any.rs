use std::time::Duration;

#[macro_export]
macro_rules! wait_for_any {
    ($wait:expr, $test:expr, $check:expr, $assert:expr) => {{
        loop {
            let o = $test;
            if !$wait.wait_any().await || $check(&o) {
                $assert(o);
                break;
            }
        }
    }};
}

#[macro_export]
macro_rules! wait_for_any_10s {
    ($test:expr, $check:expr, $assert:expr) => {
        let mut wait_for = $crate::test_utils::WaitForAny::ten_s();
        $crate::wait_for_any!(wait_for, $test, $check, $assert)
    };
}

#[macro_export]
macro_rules! wait_for_any_1m {
    ($test:expr, $check:expr, $assert:expr) => {
        let mut wait_for = $crate::test_utils::WaitForAny::one_m();
        $crate::wait_for_any!(wait_for, $test, $check, $assert)
    };
}

#[macro_export]
macro_rules! assert_retry {
    ($wait:expr, $test:expr, $check:expr $(, $reason:literal)?) => {
        $crate::wait_for_any!($wait, $test, $check, |x| assert!(x $(, $reason)?))
    };
}

#[macro_export]
macro_rules! assert_eq_retry {
    ($wait:expr, $test:expr, $check:expr $(, $reason:literal)?) => {
        $crate::wait_for_any!($wait, $test, |x| x == &$check, |x| assert_eq!(x, $check $(, $reason)?))
    };
}

#[macro_export]
macro_rules! assert_retry_10s {
    ($test:expr, $check:expr $(, $reason:literal)? $(,)?) => {
        let mut wait_for = $crate::test_utils::WaitForAny::ten_s();
        $crate::assert_retry!(wait_for, $test, $check  $(, $reason:literal)?)
    };
}

#[macro_export]
macro_rules! assert_eq_retry_10s {
    ($test:expr, $check:expr $(, $reason:literal)? $(,)?) => {
        let mut wait_for = $crate::test_utils::WaitForAny::ten_s();
        $crate::assert_eq_retry!(wait_for, $test, $check  $(, $reason:literal)?)
    };
}

#[macro_export]
macro_rules! assert_retry_1m {
    ($test:expr, $check:expr $(, $reason:literal)? $(,)?) => {
        let mut wait_for = $crate::test_utils::WaitForAny::one_m();
        $crate::assert_retry!(wait_for, $test, $check  $(, $reason:literal)?)
    };
}

#[macro_export]
macro_rules! assert_eq_retry_1m {
    ($test:expr, $check:expr $(, $reason:literal)? $(,)?) => {
        let mut wait_for = $crate::test_utils::WaitForAny::one_m();
        $crate::assert_eq_retry!(wait_for, $test, $check  $(, $reason:literal)?)
    };
}

#[derive(Debug, Clone)]
/// Generic waiting for some test property to
/// be true. This allows early exit from waiting when
/// the condition becomes true but will wait up to a
/// maximum if the condition is not true.
pub struct WaitForAny {
    num_attempts: usize,
    attempt: usize,
    delay: Duration,
}

impl WaitForAny {
    /// Create a new wait for from a number of attempts and delay in between attempts
    pub fn new(num_attempts: usize, delay: Duration) -> Self {
        Self {
            num_attempts,
            attempt: 0,
            delay,
        }
    }

    /// Wait for 10s checking every 100ms.
    pub fn ten_s() -> Self {
        const DELAY_PER_ATTEMPT: std::time::Duration = std::time::Duration::from_millis(100);
        Self::new(100, DELAY_PER_ATTEMPT)
    }

    /// Wait for 1 minute checking every 500ms.
    pub fn one_m() -> Self {
        const DELAY_PER_ATTEMPT: std::time::Duration = std::time::Duration::from_millis(500);
        Self::new(120, DELAY_PER_ATTEMPT)
    }

    /// Wait for some time before trying again.
    /// Will return false when you should stop waiting.
    #[tracing::instrument(skip(self))]
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
