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

    /// Get Duration until timeout expires.
    pub fn time_remaining(&self) -> std::time::Duration {
        self.0.saturating_duration_since(std::time::Instant::now())
    }

    /// Has this timeout expired?
    pub fn is_expired(&self) -> bool {
        self.0 <= std::time::Instant::now()
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
        tokio::time::delay_for(std::time::Duration::from_millis(2)).await;
        assert!(t.time_remaining().as_micros() == 0);
        assert!(t.is_expired());
    }
}
