use rand::Rng;
use std::future::Future;
use std::time::Duration;

/// Controls how a resilient connection backs off between reconnect attempts.
///
/// Reconnection never gives up. A connection that cannot be re-established —
/// because the conductor is down, or because its app interface rejects the
/// configured origin — keeps retrying at `max_delay` until it succeeds or the
/// connection handle is dropped.
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// Delay before the first retry.
    pub initial_delay: Duration,
    /// Upper bound on the delay between retries.
    pub max_delay: Duration,
    /// Consecutive failures after which attempts are logged at `error` rather
    /// than `warn`, so operator alerting can fire.
    pub escalate_after: u32,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            escalate_after: 5,
        }
    }
}

/// Computes the delay before the retry that follows `attempt`.
///
/// The delay doubles per attempt, is capped at [`ReconnectConfig::max_delay`],
/// and carries up to 10% jitter so that many clients reconnecting to the same
/// conductor do not synchronise.
pub(crate) fn delay_for_attempt(attempt: u32, config: &ReconnectConfig) -> Duration {
    let factor = 1u32.checked_shl(attempt.min(20)).unwrap_or(u32::MAX);
    let capped = config
        .initial_delay
        .saturating_mul(factor)
        .min(config.max_delay);

    let jitter_ceiling = capped.mul_f64(0.1).as_nanos() as u64;
    let jitter = if jitter_ceiling == 0 {
        0
    } else {
        rand::thread_rng().gen_range(0..=jitter_ceiling)
    };

    capped.saturating_add(Duration::from_nanos(jitter))
}

/// Calls `factory` until it succeeds, backing off between attempts.
///
/// This never returns an error. Cancel it by dropping the future, which is how
/// a resilient connection stops reconnecting when its handle goes away.
pub(crate) async fn connect_with_backoff<F, Fut, T, E>(
    label: &str,
    config: &ReconnectConfig,
    factory: F,
) -> T
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut attempt: u32 = 0;
    loop {
        match factory().await {
            Ok(value) => {
                if attempt > 0 {
                    tracing::info!(connection = label, attempts = attempt, "reconnected");
                }
                return value;
            }
            Err(err) => {
                let delay = delay_for_attempt(attempt, config);
                if attempt >= config.escalate_after {
                    tracing::error!(
                        connection = label,
                        attempt,
                        ?delay,
                        error = %err,
                        "reconnect failing persistently, operator attention needed"
                    );
                } else {
                    tracing::warn!(connection = label, attempt, ?delay, error = %err, "reconnect attempt failed");
                }
                attempt = attempt.saturating_add(1);
                tokio::time::sleep(delay).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn delay_starts_at_the_initial_delay() {
        let config = ReconnectConfig::default();
        let delay = delay_for_attempt(0, &config);
        assert!(delay >= config.initial_delay);
        assert!(delay <= config.initial_delay.mul_f64(1.1));
    }

    #[test]
    fn delay_doubles_per_attempt() {
        let config = ReconnectConfig::default();
        assert!(delay_for_attempt(1, &config) >= config.initial_delay * 2);
        assert!(delay_for_attempt(2, &config) >= config.initial_delay * 4);
    }

    #[test]
    fn delay_is_capped_at_the_maximum() {
        let config = ReconnectConfig::default();
        let delay = delay_for_attempt(20, &config);
        assert!(delay >= config.max_delay);
        assert!(delay <= config.max_delay.mul_f64(1.1));
    }

    #[test]
    fn delay_does_not_overflow_at_large_attempt_counts() {
        let config = ReconnectConfig::default();
        let delay = delay_for_attempt(u32::MAX, &config);
        assert!(delay <= config.max_delay.mul_f64(1.1));
    }

    #[tokio::test(start_paused = true)]
    async fn retries_until_the_factory_succeeds() {
        let attempts = Arc::new(AtomicU32::new(0));

        let value = connect_with_backoff("test", &ReconnectConfig::default(), || {
            let attempts = attempts.clone();
            async move {
                if attempts.fetch_add(1, Ordering::SeqCst) < 2 {
                    Err("not yet")
                } else {
                    Ok(42u32)
                }
            }
        })
        .await;

        assert_eq!(value, 42);
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }
}
