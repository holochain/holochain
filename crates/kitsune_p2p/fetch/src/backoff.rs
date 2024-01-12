use std::time::{Duration, Instant};

use backon::{FibonacciBackoff, FibonacciBuilder, BackoffBuilder};

struct FetchItemBackoff {
    backoff: FibonacciBackoff,
    min_delay: Duration,
    current_wait: Duration,
    started_wait_at: Instant,
    new_sources: u32,
    expired: bool
}

impl FetchItemBackoff {
    pub fn new(initial_delay: Duration) -> Self {
        let backoff = FibonacciBuilder::default()
            .with_jitter()
            .with_min_delay(initial_delay)
            .with_max_delay(std::time::Duration::from_secs(6 * 60 * 60))
            .with_max_times(15)
            .build();

        Self {
            backoff,
            min_delay: initial_delay,
            current_wait: Duration::ZERO,
            started_wait_at: Instant::now(),
            new_sources: 0,
            expired: false,
        }
    }

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

    pub fn is_expired(&self) -> bool {
        self.expired
    }

    pub fn new_sources(&mut self) {
        self.current_wait = Duration::ZERO;
        self.new_sources += 1;
    }

    fn advance(&mut self) {
        if self.new_sources > 0 {
            self.new_sources -= 1;
        }

        if self.new_sources > 0 {
            self.current_wait = self.min_delay;
            self.started_wait_at = Instant::now();
        } else {
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
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use super::FetchItemBackoff;
    
    #[test]
    fn backoff_is_ready_at_initialisation() {
        let mut backoff = FetchItemBackoff::new(Duration::from_secs(1));
        assert!(backoff.is_ready());
    }

    #[test]
    fn first_delay_is_initial_delay() {
        let initial_delay = Duration::from_secs(1);
        let mut backoff = FetchItemBackoff::new(initial_delay);
        assert!(backoff.is_ready());

        // After ready check, should delay and so not be ready
        assert!(!backoff.is_ready());

        // Account for jitter and check that the delay is roughly the initial delay
        assert!(initial_delay <= backoff.current_wait && backoff.current_wait < initial_delay * 2);
    }
}
