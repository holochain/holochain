use crate::*;

use std::sync::atomic::{AtomicU64, Ordering};

/// Kitsune Backoff
#[derive(Debug, Clone)]
pub struct KitsuneBackoff {
    timeout: KitsuneTimeout,
    cur_ms: Arc<AtomicU64>,
    max_ms: u64,
}

impl KitsuneBackoff {
    /// backoff constructor
    pub fn new(timeout: KitsuneTimeout, initial_ms: u64, max_ms: u64) -> Self {
        let cur_ms = Arc::new(AtomicU64::new(initial_ms));
        Self {
            timeout,
            cur_ms,
            max_ms,
        }
    }

    /// Wait for the current backoff time (but not longer than timeout expiry)
    /// then increment the current wait duration
    pub async fn wait(&self) {
        // get the current val
        let cur = self.cur_ms.load(Ordering::Relaxed);

        // it's ok if our exponential isn't *exactly* exponential
        // due to multiple threads increasing this simultaneously
        self.cur_ms.fetch_add(cur, Ordering::Relaxed);

        // cap this at max and/or time remaining
        let cur = std::cmp::min(
            cur,
            std::cmp::min(
                self.max_ms,
                // add 1ms to our time remaining so we don't have a weird
                // race condition where we don't wait at all, but our
                // timer hasn't quite expired yet...
                self.timeout.time_remaining().as_millis() as u64 + 1,
            ),
        );

        // wait that time
        if cur > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(cur)).await;
        }
    }
}

/// Kitsune Timeout
#[derive(Debug, Clone, Copy)]
pub struct KitsuneTimeout(std::time::Instant);

impl KitsuneTimeout {
    /// Create a new timeout for duration in the future.
    pub fn new(duration: std::time::Duration) -> Self {
        Self(std::time::Instant::now().checked_add(duration).unwrap())
    }

    /// Convenience fn to create a new timeout for an amount of milliseconds.
    pub fn from_millis(millis: u64) -> Self {
        Self::new(std::time::Duration::from_millis(millis))
    }

    /// Generate a backoff instance bound to this timeout
    pub fn backoff(&self, initial_ms: u64, max_ms: u64) -> KitsuneBackoff {
        KitsuneBackoff::new(*self, initial_ms, max_ms)
    }

    /// Get Duration until timeout expires.
    pub fn time_remaining(&self) -> std::time::Duration {
        self.0.saturating_duration_since(std::time::Instant::now())
    }

    /// Has this timeout expired?
    pub fn is_expired(&self) -> bool {
        self.0 <= std::time::Instant::now()
    }

    /// `Ok(())` if not expired, `Err(KitsuneError::TimedOut)` if expired.
    pub fn ok(&self) -> KitsuneResult<()> {
        if self.is_expired() {
            Err(KitsuneErrorKind::TimedOut.into())
        } else {
            Ok(())
        }
    }

    /// Wrap a future with one that will timeout when this timeout expires.
    pub fn mix<'a, 'b, R, F>(
        &'a self,
        f: F,
    ) -> impl std::future::Future<Output = KitsuneResult<R>> + 'b + Send
    where
        R: 'b,
        F: std::future::Future<Output = KitsuneResult<R>> + 'b + Send,
    {
        let time_remaining = self.time_remaining();
        async move {
            match tokio::time::timeout(time_remaining, f).await {
                Ok(r) => r,
                Err(_) => Err(KitsuneErrorKind::TimedOut.into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_kitsune_timeout() {
        let t = KitsuneTimeout::new(std::time::Duration::from_millis(40));
        assert!(t.time_remaining().as_millis() > 0);
        assert!(!t.is_expired());
    }

    #[tokio::test]
    async fn expired_kitsune_timeout() {
        let t = KitsuneTimeout::new(std::time::Duration::from_millis(1));
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        assert!(t.time_remaining().as_micros() == 0);
        assert!(t.is_expired());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn kitsune_backoff() {
        let t = KitsuneTimeout::from_millis(100);
        let mut times = Vec::new();
        let start = std::time::Instant::now();
        let bo = t.backoff(2, 15);
        while !t.is_expired() {
            times.push(start.elapsed().as_millis() as u64);
            bo.wait().await;
        }
        println!("{:?}", times);
        assert!(times.len() > 4);
        assert!(times.len() < 20);
    }
}
