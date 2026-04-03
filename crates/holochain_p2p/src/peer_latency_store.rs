use kitsune2_api::Url;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

/// Maximum number of latency samples kept per peer URL in the rolling window.
const MAX_SAMPLES: usize = 10;

/// Duration after which a latency estimate is considered expired and needs re-pinging.
const EXPIRY_DURATION: Duration = Duration::from_secs(60 * 60);

/// Rolling-window latency estimate for a single peer URL.
///
/// Maintains up to [`MAX_SAMPLES`] RTT samples and a cached average.
/// Entries expire after [`EXPIRY_DURATION`] based on `recorded_at`.
#[derive(Debug)]
pub(crate) struct LatencyEstimate {
    samples: VecDeque<Duration>,
    average: Duration,
    recorded_at: Instant,
}

impl LatencyEstimate {
    /// Creates a new estimate seeded with a single sample.
    fn new(sample: Duration) -> Self {
        let mut samples = VecDeque::with_capacity(MAX_SAMPLES);
        samples.push_back(sample);
        Self {
            samples,
            average: sample,
            recorded_at: Instant::now(),
        }
    }

    /// Returns `true` if this estimate is older than [`EXPIRY_DURATION`].
    fn is_expired(&self) -> bool {
        self.recorded_at.elapsed() >= EXPIRY_DURATION
    }
}

/// In-memory store for peer latency estimates, keyed by transport URL.
///
/// Tracks rolling-average RTT per peer and coordinates in-flight ping
/// deduplication via [`begin_ping`](Self::begin_ping) / [`finish_ping`](Self::finish_ping).
#[derive(Debug, Default)]
pub(crate) struct PeerLatencyStore {
    estimates: HashMap<Url, LatencyEstimate>,
    in_flight_pings: HashSet<Url>,
}

impl PeerLatencyStore {
    /// Creates an empty store.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Records a new RTT sample for the given URL.
    ///
    /// Maintains a rolling window of at most [`MAX_SAMPLES`] entries,
    /// evicting the oldest when full. Recalculates the cached average
    /// and updates `recorded_at`.
    pub(crate) fn record_sample(&mut self, url: Url, rtt: Duration) {
        match self.estimates.get_mut(&url) {
            Some(entry) => {
                if entry.samples.len() == MAX_SAMPLES {
                    let _ = entry.samples.pop_front();
                }
                entry.samples.push_back(rtt);
                entry.recorded_at = Instant::now();

                let total = entry
                    .samples
                    .iter()
                    .copied()
                    .fold(Duration::ZERO, |acc, sample| acc + sample);
                entry.average = total / (entry.samples.len() as u32);
            }
            None => {
                self.estimates.insert(url, LatencyEstimate::new(rtt));
            }
        }
    }

    /// Returns the rolling-average latency for the given URL if the entry
    /// exists and has not expired. Returns `None` otherwise.
    pub(crate) fn get_latency(&self, url: &Url) -> Option<Duration> {
        self.estimates.get(url).and_then(|entry| {
            if entry.is_expired() {
                None
            } else {
                Some(entry.average)
            }
        })
    }

    /// Returns the rolling-average latency even if the entry has expired.
    /// Used as a soft fallback during re-ping to avoid sudden weight changes.
    pub(crate) fn get_latency_including_stale(&self, url: &Url) -> Option<Duration> {
        self.estimates.get(url).map(|entry| entry.average)
    }

    /// Returns `true` if the URL has no entry or its entry has expired,
    /// indicating that a fresh round of pings should be scheduled.
    pub(crate) fn needs_ping(&self, url: &Url) -> bool {
        self.estimates
            .get(url)
            .is_none_or(LatencyEstimate::is_expired)
    }

    /// Attempts to mark a URL as having an in-flight ping.
    ///
    /// Returns `true` if the ping should proceed (URL needs pinging and no
    /// ping is already in flight). Returns `false` if the URL already has a
    /// fresh estimate or another ping task is running.
    pub(crate) fn begin_ping(&mut self, url: &Url) -> bool {
        if !self.needs_ping(url) || self.in_flight_pings.contains(url) {
            return false;
        }

        self.in_flight_pings.insert(url.clone())
    }

    /// Removes the in-flight marker for a URL after pinging completes.
    pub(crate) fn finish_ping(&mut self, url: &Url) {
        self.in_flight_pings.remove(url);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_url(name: &str) -> Url {
        Url::from_str(format!("ws://test-{name}:1234")).unwrap()
    }

    #[test]
    fn get_latency_returns_none_for_unknown_peer() {
        let store = PeerLatencyStore::new();
        assert!(store.get_latency(&test_url("a")).is_none());
    }

    #[test]
    fn needs_ping_returns_true_for_unknown_peer() {
        let store = PeerLatencyStore::new();
        assert!(store.needs_ping(&test_url("a")));
    }

    #[test]
    fn record_sample_and_get_latency() {
        let mut store = PeerLatencyStore::new();
        let url = test_url("a");
        store.record_sample(url.clone(), Duration::from_millis(100));
        store.record_sample(url.clone(), Duration::from_millis(200));

        assert_eq!(store.get_latency(&url), Some(Duration::from_millis(150)));
    }

    #[test]
    fn begin_ping_prevents_duplicate_in_flight_work() {
        let mut store = PeerLatencyStore::new();
        let url = test_url("a");

        assert!(store.begin_ping(&url));
        assert!(!store.begin_ping(&url));

        store.finish_ping(&url);

        assert!(store.begin_ping(&url));
    }

    #[test]
    fn needs_ping_returns_false_for_known_peer() {
        let mut store = PeerLatencyStore::new();
        let url = test_url("a");
        store.record_sample(url.clone(), Duration::from_millis(50));

        assert!(!store.needs_ping(&url));
        assert!(!store.begin_ping(&url));
    }

    #[test]
    fn rolling_window_evicts_oldest_at_max_samples() {
        let mut store = PeerLatencyStore::new();
        let url = test_url("a");

        for _ in 0..MAX_SAMPLES {
            store.record_sample(url.clone(), Duration::from_millis(100));
        }

        assert_eq!(store.get_latency(&url), Some(Duration::from_millis(100)));

        store.record_sample(url.clone(), Duration::from_millis(200));

        assert_eq!(store.get_latency(&url), Some(Duration::from_millis(110)));
    }

    #[test]
    fn expired_entry_returns_none_and_needs_ping() {
        let mut store = PeerLatencyStore::new();
        let url = test_url("a");
        store.record_sample(url.clone(), Duration::from_millis(50));

        store.estimates.get_mut(&url).unwrap().recorded_at =
            Instant::now() - Duration::from_secs(2 * 60 * 60);

        assert!(store.get_latency(&url).is_none());
        assert!(store.needs_ping(&url));
    }

    #[test]
    fn get_latency_including_stale_returns_expired_average() {
        let mut store = PeerLatencyStore::new();
        let url = test_url("a");
        store.record_sample(url.clone(), Duration::from_millis(50));

        store.estimates.get_mut(&url).unwrap().recorded_at =
            Instant::now() - Duration::from_secs(2 * 60 * 60);

        assert!(store.get_latency(&url).is_none());
        assert_eq!(
            store.get_latency_including_stale(&url),
            Some(Duration::from_millis(50))
        );
    }
}
