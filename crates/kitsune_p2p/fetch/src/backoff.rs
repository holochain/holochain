use std::time::{Duration, Instant};

use backon::{BackoffBuilder, FibonacciBackoff, FibonacciBuilder};

/// a struct
#[derive(Debug)]
pub struct FetchBackoff {
    backoff: FibonacciBackoff,
    current_wait: Duration,
    started_wait_at: Instant,
    expired: bool,
}

impl FetchBackoff {
    /// create it
    pub fn new(initial_delay: Duration) -> Self {
        let backoff = FibonacciBuilder::default()
            .with_jitter()
            .with_min_delay(initial_delay)
            .with_max_delay(std::time::Duration::from_secs(6 * 60 * 60))
            .with_max_times(15)
            .build();

        Self {
            backoff,
            current_wait: Duration::ZERO,
            started_wait_at: Instant::now(),
            expired: false,
        }
    }

    /// ready
    pub fn is_ready(&mut self) -> bool {
        if self.expired {
            return false;
        }

        if self.started_wait_at.elapsed() >= self.current_wait {
            self.advance();
            true
        } else {
            false
        }
    }

    /// expired
    pub fn is_expired(&self) -> bool {
        self.expired
    }

    fn advance(&mut self) {
        match self.backoff.next() {
            Some(d) => {
                self.current_wait = d;
                self.started_wait_at = Instant::now();
            }
            None => {
                self.expired = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FetchBackoff;
    use std::time::Duration;

    #[test]
    fn backoff_is_ready_at_initialisation() {
        let mut backoff = FetchBackoff::new(Duration::from_secs(1));
        assert!(backoff.is_ready());
    }

    #[test]
    fn first_delay_is_initial_delay() {
        let initial_delay = Duration::from_secs(1);
        let mut backoff = FetchBackoff::new(initial_delay);
        assert!(backoff.is_ready());

        // After ready check, should delay and so not be ready
        assert!(!backoff.is_ready());

        // Account for jitter and check that the delay is roughly the initial delay
        assert!(initial_delay <= backoff.current_wait && backoff.current_wait < initial_delay * 2);
    }

    #[test]
    fn number_of_tries_is_limited() {
        let mut backoff = FetchBackoff::new(Duration::from_nanos(1));
        assert!(backoff.is_ready());
        assert!(!backoff.is_expired());

        let mut num_tries = 0;
        for _ in 0..1000 {
            if backoff.is_expired() {
                break;
            }

            while !backoff.is_ready() {
                std::thread::sleep(Duration::from_nanos(10));
            }

            num_tries += 1;
        }

        assert!(backoff.is_expired());
        assert!(!backoff.is_ready());
        assert_eq!(15, num_tries);
    }
}
