use futures::stream::FuturesUnordered;
use futures::StreamExt;
use kitsune2_api::{DynSpace, Url};
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::AbortHandle;

/// Maximum number of latency samples kept per peer URL in the rolling window.
const MAX_SAMPLES: usize = 10;

/// Estimates are considered stale after `recorded_at + EXPIRY_DURATION`.
const EXPIRY_DURATION: Duration = Duration::from_secs(60 * 60);

/// Entries older than this are permanently evicted from the store to bound memory.
/// Set to twice [`EXPIRY_DURATION`] so stale fallback data is available during re-ping.
const EVICTION_DURATION: Duration = Duration::from_secs(2 * 60 * 60);

/// How far before [`EXPIRY_DURATION`] to schedule a proactive refresh ping.
const REFRESH_BUFFER: Duration = Duration::from_secs(10 * 60);

/// Number of sequential ping samples to collect per peer URL.
const PING_SAMPLE_COUNT: usize = 10;

/// Maximum number of peer URLs to ping concurrently in the background worker.
const MAX_CONCURRENT_PINGS: usize = 8;

/// After this many consecutive all-failure ping rounds, a peer is
/// considered to have failed pings and will not be pinged again until
/// re-touched.
const CONSECUTIVE_FAILURE_THRESHOLD: u32 = 3;

/// Default tick interval for the worker when no peer is immediately due.
const WORKER_TICK_INTERVAL: Duration = Duration::from_secs(60);

/// Function type for sending a single ping and measuring RTT.
///
/// The actor provides an implementation that sends a `PingReq` wire message
/// and waits for the `PingRes`, returning the measured round-trip time.
pub(crate) type PingFn = Arc<
    dyn Fn(DynSpace, Url) -> Pin<Box<dyn Future<Output = Option<Duration>> + Send>> + Send + Sync,
>;

/// Rolling-window latency estimate for a single peer URL.
///
/// Maintains up to [`MAX_SAMPLES`] RTT samples and a cached average.
/// Entries are considered stale after `recorded_at + EXPIRY_DURATION`.
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

    /// Appends a sample to the rolling window, evicting the oldest when
    /// the window is full, then recomputes the cached average and
    /// updates `recorded_at`.
    fn push_sample(&mut self, rtt: Duration) {
        if self.samples.len() == MAX_SAMPLES {
            let _ = self.samples.pop_front();
        }
        self.samples.push_back(rtt);
        self.recorded_at = Instant::now();

        let total = self
            .samples
            .iter()
            .copied()
            .fold(Duration::ZERO, |acc, sample| acc + sample);
        self.average = total / (self.samples.len() as u32);
    }

    /// Returns `true` if this estimate is older than [`EXPIRY_DURATION`]
    /// relative to the given reference instant.
    fn is_expired_at(&self, now: Instant) -> bool {
        now.duration_since(self.recorded_at) >= EXPIRY_DURATION
    }

    /// Returns `true` when this estimate is within [`REFRESH_BUFFER`] of
    /// [`EXPIRY_DURATION`], indicating it should be proactively refreshed
    /// before it expires.
    fn is_due_for_refresh(&self, now: Instant) -> bool {
        let refresh_threshold = EXPIRY_DURATION.saturating_sub(REFRESH_BUFFER);
        now.duration_since(self.recorded_at) >= refresh_threshold
    }

    /// Returns the selection weight for this estimate: `1 / latency_ms`.
    ///
    /// Lower-latency peers get higher weights. Latency is clamped to a
    /// minimum of 1 ms to avoid division by zero.
    pub(crate) fn weight(&self) -> f64 {
        1.0 / (self.average.as_secs_f64() * 1000.0).max(1.0)
    }
}

/// Pure latency data: estimates and failure bookkeeping. No async, no tasks.
#[derive(Debug, Default)]
pub(crate) struct LatencyData {
    estimates: HashMap<Url, LatencyEstimate>,
    consecutive_failures: HashMap<Url, u32>,
}

impl LatencyData {
    /// Records a new RTT sample for the given URL.
    ///
    /// Maintains a rolling window of at most [`MAX_SAMPLES`] entries,
    /// evicting the oldest when full. Recalculates the cached average
    /// and updates `recorded_at`. Also clears any consecutive failure count.
    pub(crate) fn record_sample(&mut self, url: Url, rtt: Duration) {
        self.consecutive_failures.remove(&url);

        match self.estimates.get_mut(&url) {
            Some(entry) => entry.push_sample(rtt),
            None => {
                self.estimates.insert(url, LatencyEstimate::new(rtt));
            }
        }
    }

    /// Returns the rolling-average latency for the given URL if the entry
    /// exists and has not expired. Returns `None` otherwise.
    #[cfg(test)]
    fn get_latency(&self, url: &Url) -> Option<Duration> {
        self.get_latency_at(url, Instant::now())
    }

    /// Returns the rolling-average latency relative to the given reference
    /// instant. Used in tests to avoid platform-dependent `Instant` arithmetic.
    #[cfg(test)]
    fn get_latency_at(&self, url: &Url, now: Instant) -> Option<Duration> {
        self.estimates.get(url).and_then(|entry| {
            if entry.is_expired_at(now) {
                None
            } else {
                Some(entry.average)
            }
        })
    }

    /// Returns the rolling-average latency even if the entry has expired.
    /// Used as a soft fallback during re-ping to avoid sudden weight changes.
    #[cfg(test)]
    fn get_latency_including_stale(&self, url: &Url) -> Option<Duration> {
        self.estimates.get(url).map(|entry| entry.average)
    }

    /// Returns the selection weight for any known URL.
    ///
    /// Returns `None` if the URL has never been seen. Freshness is not
    /// checked here — the worker proactively refreshes entries before
    /// [`EXPIRY_DURATION`] via [`REFRESH_BUFFER`], and [`evict_stale`]
    /// removes entries past [`EVICTION_DURATION`]. Any entry still in
    /// the store is considered usable for weighting.
    pub(crate) fn get_weight(&self, url: &Url) -> Option<f64> {
        self.estimates.get(url).map(|entry| entry.weight())
    }

    /// Records a full-round ping failure for a URL, incrementing the
    /// consecutive failure counter.
    pub(crate) fn record_failure(&mut self, url: &Url) {
        *self.consecutive_failures.entry(url.clone()).or_insert(0) += 1;
    }

    /// Returns `true` if the URL has exceeded [`CONSECUTIVE_FAILURE_THRESHOLD`]
    /// consecutive ping failures and should not be pinged again.
    ///
    /// This is a ping-specific signal distinct from the transport-level
    /// unresponsive marker maintained in the peer meta store.
    pub(crate) fn has_failed_pings(&self, url: &Url) -> bool {
        self.consecutive_failures
            .get(url)
            .is_some_and(|&count| count >= CONSECUTIVE_FAILURE_THRESHOLD)
    }

    /// Clears the consecutive failure counter for a URL, re-enabling pings.
    fn clear_failures(&mut self, url: &Url) {
        self.consecutive_failures.remove(url);
    }

    /// Returns `true` if the URL has no estimate, an expired estimate,
    /// or an estimate due for proactive refresh.
    fn needs_ping(&self, url: &Url) -> bool {
        self.needs_ping_at(url, Instant::now())
    }

    /// Returns `true` if the URL has no estimate, an expired estimate,
    /// or an estimate due for proactive refresh, relative to the given
    /// reference instant.
    fn needs_ping_at(&self, url: &Url, now: Instant) -> bool {
        self.estimates
            .get(url)
            .is_none_or(|e| e.is_expired_at(now) || e.is_due_for_refresh(now))
    }

    /// Removes entries whose `recorded_at` is older than [`EVICTION_DURATION`],
    /// bounding memory growth under peer churn.
    fn evict_stale(&mut self) {
        self.evict_stale_at(Instant::now());
    }

    /// Removes entries whose `recorded_at` is older than [`EVICTION_DURATION`]
    /// relative to the given reference instant.
    fn evict_stale_at(&mut self, now: Instant) {
        self.estimates
            .retain(|_, entry| now.duration_since(entry.recorded_at) < EVICTION_DURATION);
    }
}

/// Request to register or refresh a peer URL for latency tracking.
struct TouchRequest {
    url: Url,
    space: DynSpace,
}

/// Per-URL bookkeeping inside the background worker.
struct PeerWorkState {
    space: DynSpace,
    last_touched_at: Instant,
}

/// Active latency-estimation service.
///
/// Owns a single background Tokio task that pings peers on a schedule.
/// The actor interacts only through [`touch`](Self::touch) and
/// [`store`](Self::store).
pub(crate) struct PeerLatencyService {
    touch_tx: mpsc::UnboundedSender<TouchRequest>,
    store: Arc<Mutex<LatencyData>>,
    abort_handle: AbortHandle,
}

impl PeerLatencyService {
    /// Spawns the background worker task and returns the service handle.
    pub(crate) fn new(ping_fn: PingFn) -> Self {
        let (touch_tx, touch_rx) = mpsc::unbounded_channel();
        let store = Arc::new(Mutex::new(LatencyData::default()));
        let task_store = Arc::clone(&store);
        let handle = tokio::spawn(run_worker(touch_rx, task_store, ping_fn));
        let abort_handle = handle.abort_handle();
        Self {
            touch_tx,
            store,
            abort_handle,
        }
    }

    /// Notifies the service that a peer URL was encountered.
    ///
    /// If the URL is new, the worker will schedule pings for it.
    /// If the URL previously had failed pings, it is re-activated.
    pub(crate) fn touch(&self, url: Url, space: DynSpace) {
        let _ = self.touch_tx.send(TouchRequest { url, space });
    }

    /// Returns a shared handle to the latency data for read access
    /// (e.g., weighted selection).
    pub(crate) fn store(&self) -> Arc<Mutex<LatencyData>> {
        Arc::clone(&self.store)
    }
}

impl Drop for PeerLatencyService {
    fn drop(&mut self) {
        self.abort_handle.abort();
    }
}

/// Computes how long the worker should sleep before the next peer is due
/// for a ping.
fn compute_next_wake_duration(
    peer_state: &HashMap<Url, PeerWorkState>,
    store: &Mutex<LatencyData>,
) -> Duration {
    if peer_state.is_empty() {
        return WORKER_TICK_INTERVAL;
    }

    let data = store.lock().expect("latency data lock poisoned");
    let now = Instant::now();
    let mut min_wait = WORKER_TICK_INTERVAL;

    for url in peer_state.keys() {
        if data.has_failed_pings(url) {
            continue;
        }
        match data.estimates.get(url) {
            None => return Duration::ZERO,
            Some(est) => {
                let refresh_at = est.recorded_at + EXPIRY_DURATION.saturating_sub(REFRESH_BUFFER);
                if now >= refresh_at {
                    return Duration::ZERO;
                }
                let wait = refresh_at - now;
                if wait < min_wait {
                    min_wait = wait;
                }
            }
        }
    }

    min_wait
}

/// Prunes worker-side state for URLs whose estimate has been evicted and
/// that haven't been touched within [`EVICTION_DURATION`].
///
/// Also drops orphan `consecutive_failures` entries for pruned URLs to keep
/// failure bookkeeping bounded under peer churn.
fn prune_peer_state_at(
    peer_state: &mut HashMap<Url, PeerWorkState>,
    data: &mut LatencyData,
    now: Instant,
) {
    peer_state.retain(|url, state| {
        data.estimates.contains_key(url)
            || now.duration_since(state.last_touched_at) < EVICTION_DURATION
    });
    data.consecutive_failures
        .retain(|url, _| peer_state.contains_key(url));
}

/// Background worker loop that schedules and executes pings.
async fn run_worker(
    mut touch_rx: mpsc::UnboundedReceiver<TouchRequest>,
    store: Arc<Mutex<LatencyData>>,
    ping_fn: PingFn,
) {
    let mut peer_state: HashMap<Url, PeerWorkState> = HashMap::new();

    loop {
        let next_wait = compute_next_wake_duration(&peer_state, &store);

        tokio::select! {
            biased;

            msg = touch_rx.recv() => {
                match msg {
                    Some(touch) => {
                        store
                            .lock()
                            .expect("latency data lock poisoned")
                            .clear_failures(&touch.url);
                        let now = Instant::now();
                        peer_state
                            .entry(touch.url)
                            .and_modify(|state| {
                                state.space = touch.space.clone();
                                state.last_touched_at = now;
                            })
                            .or_insert_with(|| PeerWorkState {
                                space: touch.space,
                                last_touched_at: now,
                            });
                    }
                    None => break,
                }
            }

            _ = tokio::time::sleep(next_wait) => {
                ping_due_peers(&peer_state, &store, &ping_fn).await;
                let mut data = store.lock().expect("latency data lock poisoned");
                data.evict_stale();
                prune_peer_state_at(&mut peer_state, &mut data, Instant::now());
            }
        }
    }
}

/// Finds peers that are due for pinging and pings them with bounded concurrency.
async fn ping_due_peers(
    peer_state: &HashMap<Url, PeerWorkState>,
    store: &Arc<Mutex<LatencyData>>,
    ping_fn: &PingFn,
) {
    let urls_to_ping: Vec<(Url, DynSpace)> = {
        let data = store.lock().expect("latency data lock poisoned");
        peer_state
            .iter()
            .filter(|(url, _)| !data.has_failed_pings(url) && data.needs_ping(url))
            .map(|(url, state)| (url.clone(), state.space.clone()))
            .collect()
    };

    if urls_to_ping.is_empty() {
        return;
    }

    let mut futures = FuturesUnordered::new();

    for (url, space) in urls_to_ping {
        let ping_fn = Arc::clone(ping_fn);
        let task_store = Arc::clone(store);
        futures.push(async move {
            let mut success_count = 0u32;
            for _ in 0..PING_SAMPLE_COUNT {
                match ping_fn(space.clone(), url.clone()).await {
                    Some(rtt) => {
                        task_store
                            .lock()
                            .expect("latency data lock poisoned")
                            .record_sample(url.clone(), rtt);
                        success_count += 1;
                    }
                    None => {
                        // Bail early: a failed sample usually means
                        // send_notify error or timeout, so spending
                        // the full PING_TIMEOUT budget on the remaining
                        // samples would just delay the failure round.
                        break;
                    }
                }
            }
            (url, success_count)
        });

        // Drain completed futures when we hit the concurrency limit.
        while futures.len() >= MAX_CONCURRENT_PINGS {
            if let Some((url, success_count)) = futures.next().await {
                handle_ping_result(store, &url, success_count);
            }
        }
    }

    // Drain remaining futures.
    while let Some((url, success_count)) = futures.next().await {
        handle_ping_result(store, &url, success_count);
    }
}

/// Records ping outcome: logs the result and tracks consecutive failures.
fn handle_ping_result(store: &Arc<Mutex<LatencyData>>, url: &Url, success_count: u32) {
    if success_count == 0 {
        tracing::debug!(%url, "All ping samples failed for peer");
        store
            .lock()
            .expect("latency data lock poisoned")
            .record_failure(url);
    } else {
        tracing::debug!(%url, success_count, "Ping sampling complete for peer");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_url(name: &str) -> Url {
        Url::from_str(format!("ws://test-{name}:1234")).unwrap()
    }

    /// Returns a synthetic future `Instant` that is `offset` ahead of now.
    ///
    /// Tests use this instead of trying to create an `Instant` in the past,
    /// which fails on platforms where `Instant` cannot represent times before
    /// process start (e.g., short-lived Windows processes).
    fn future_instant(offset: Duration) -> Instant {
        Instant::now() + offset
    }

    // -- LatencyEstimate tests --

    #[test]
    fn weight_inverse_of_latency_ms() {
        let est = LatencyEstimate::new(Duration::from_millis(100));
        let w = est.weight();
        // 1 / 100 = 0.01
        assert!((w - 0.01).abs() < 1e-9, "weight was {w}");
    }

    #[test]
    fn weight_clamps_zero_latency_to_one_ms() {
        let est = LatencyEstimate::new(Duration::ZERO);
        let w = est.weight();
        // 1 / 1 = 1.0
        assert!((w - 1.0).abs() < 1e-9, "weight was {w}");
    }

    #[test]
    fn is_due_for_refresh_before_threshold() {
        let est = LatencyEstimate::new(Duration::from_millis(50));
        // 40 min: not yet at 50 min threshold
        let future = future_instant(Duration::from_secs(40 * 60));
        assert!(!est.is_due_for_refresh(future));
    }

    #[test]
    fn is_due_for_refresh_at_threshold() {
        let est = LatencyEstimate::new(Duration::from_millis(50));
        // 50 min: at the refresh threshold (EXPIRY - REFRESH_BUFFER = 50 min)
        let future = future_instant(Duration::from_secs(50 * 60));
        assert!(est.is_due_for_refresh(future));
    }

    #[test]
    fn is_due_for_refresh_after_expiry() {
        let est = LatencyEstimate::new(Duration::from_millis(50));
        // 65 min: past expiry
        let future = future_instant(Duration::from_secs(65 * 60));
        assert!(est.is_due_for_refresh(future));
    }

    // -- LatencyData tests --

    #[test]
    fn get_latency_returns_none_for_unknown_peer() {
        let data = LatencyData::default();
        assert!(data.get_latency(&test_url("a")).is_none());
    }

    #[test]
    fn record_sample_and_get_latency() {
        let mut data = LatencyData::default();
        let url = test_url("a");
        data.record_sample(url.clone(), Duration::from_millis(100));
        data.record_sample(url.clone(), Duration::from_millis(200));

        assert_eq!(data.get_latency(&url), Some(Duration::from_millis(150)));
    }

    #[test]
    fn rolling_window_evicts_oldest_at_max_samples() {
        let mut data = LatencyData::default();
        let url = test_url("a");

        for _ in 0..MAX_SAMPLES {
            data.record_sample(url.clone(), Duration::from_millis(100));
        }

        assert_eq!(data.get_latency(&url), Some(Duration::from_millis(100)));

        data.record_sample(url.clone(), Duration::from_millis(200));

        assert_eq!(data.get_latency(&url), Some(Duration::from_millis(110)));
    }

    #[test]
    fn expired_entry_returns_none_and_needs_ping() {
        let mut data = LatencyData::default();
        let url = test_url("a");
        data.record_sample(url.clone(), Duration::from_millis(50));

        let future = future_instant(Duration::from_secs(2 * 60 * 60));

        assert!(data.get_latency_at(&url, future).is_none());
        assert!(data.needs_ping_at(&url, future));
    }

    #[test]
    fn get_latency_including_stale_returns_expired_average() {
        let mut data = LatencyData::default();
        let url = test_url("a");
        data.record_sample(url.clone(), Duration::from_millis(50));

        let future = future_instant(Duration::from_secs(2 * 60 * 60));

        assert!(data.get_latency_at(&url, future).is_none());
        assert_eq!(
            data.get_latency_including_stale(&url),
            Some(Duration::from_millis(50))
        );
    }

    #[test]
    fn get_weight_returns_estimate_weight() {
        let mut data = LatencyData::default();
        let url = test_url("a");
        data.record_sample(url.clone(), Duration::from_millis(100));

        let w = data.get_weight(&url).unwrap();
        assert!((w - 0.01).abs() < 1e-9, "weight was {w}");
    }

    #[test]
    fn get_weight_returns_none_for_unknown() {
        let data = LatencyData::default();
        assert!(data.get_weight(&test_url("a")).is_none());
    }

    #[test]
    fn evict_stale_removes_entries_older_than_eviction_duration() {
        let mut data = LatencyData::default();
        let fresh_url = test_url("fresh");
        let stale_url = test_url("stale");

        data.record_sample(fresh_url.clone(), Duration::from_millis(50));
        data.record_sample(stale_url.clone(), Duration::from_millis(100));

        let future = future_instant(Duration::from_secs(3 * 60 * 60));
        data.estimates.get_mut(&fresh_url).unwrap().recorded_at =
            future - Duration::from_secs(30 * 60);

        data.evict_stale_at(future);

        assert!(data.get_latency_at(&fresh_url, future).is_some());
        assert!(data.get_latency_including_stale(&stale_url).is_none());
    }

    #[test]
    fn evict_stale_keeps_entries_within_eviction_window() {
        let mut data = LatencyData::default();
        let url = test_url("a");

        data.record_sample(url.clone(), Duration::from_millis(50));

        // 1.5 hours: past EXPIRY (1h) but within EVICTION (2h).
        let future = future_instant(Duration::from_secs(90 * 60));

        data.evict_stale_at(future);

        assert!(data.get_latency_at(&url, future).is_none());
        assert!(data.get_latency_including_stale(&url).is_some());
    }

    #[test]
    fn needs_ping_returns_true_for_unknown_peer() {
        let data = LatencyData::default();
        assert!(data.needs_ping(&test_url("a")));
    }

    #[test]
    fn needs_ping_returns_false_for_fresh_peer() {
        let mut data = LatencyData::default();
        let url = test_url("a");
        data.record_sample(url.clone(), Duration::from_millis(50));
        assert!(!data.needs_ping(&url));
    }

    #[test]
    fn needs_ping_returns_true_near_expiry() {
        let mut data = LatencyData::default();
        let url = test_url("a");
        data.record_sample(url.clone(), Duration::from_millis(50));

        // 55 min: past the refresh threshold (50 min)
        let future = future_instant(Duration::from_secs(55 * 60));
        assert!(data.needs_ping_at(&url, future));
    }

    // -- Ping-failure detection tests --

    #[test]
    fn has_failed_pings_after_consecutive_failures() {
        let mut data = LatencyData::default();
        let url = test_url("a");

        for _ in 0..CONSECUTIVE_FAILURE_THRESHOLD {
            assert!(!data.has_failed_pings(&url));
            data.record_failure(&url);
        }
        assert!(data.has_failed_pings(&url));
    }

    #[test]
    fn record_sample_clears_failures() {
        let mut data = LatencyData::default();
        let url = test_url("a");

        for _ in 0..CONSECUTIVE_FAILURE_THRESHOLD - 1 {
            data.record_failure(&url);
        }
        assert!(!data.has_failed_pings(&url));

        data.record_sample(url.clone(), Duration::from_millis(50));
        assert!(!data.has_failed_pings(&url));
        assert_eq!(data.consecutive_failures.get(&url), None);
    }

    #[test]
    fn clear_failures_reactivates_peer_with_failed_pings() {
        let mut data = LatencyData::default();
        let url = test_url("a");

        for _ in 0..CONSECUTIVE_FAILURE_THRESHOLD {
            data.record_failure(&url);
        }
        assert!(data.has_failed_pings(&url));

        data.clear_failures(&url);
        assert!(!data.has_failed_pings(&url));
    }

    // -- PeerLatencyService integration tests --

    /// Minimal mock of [`kitsune2_api::Space`] for testing.
    /// The ping function in tests ignores the space, so all methods
    /// are stubs that panic if called.
    mod mock_space {
        use kitsune2_api::*;
        use std::sync::Arc;

        #[derive(Debug)]
        pub(super) struct StubSpace;

        impl Space for StubSpace {
            fn peer_store(&self) -> &DynPeerStore {
                unimplemented!("stub")
            }
            fn local_agent_store(&self) -> &DynLocalAgentStore {
                unimplemented!("stub")
            }
            fn op_store(&self) -> &DynOpStore {
                unimplemented!("stub")
            }
            fn fetch(&self) -> &DynFetch {
                unimplemented!("stub")
            }
            fn publish(&self) -> &DynPublish {
                unimplemented!("stub")
            }
            fn gossip(&self) -> &DynGossip {
                unimplemented!("stub")
            }
            fn peer_meta_store(&self) -> &DynPeerMetaStore {
                unimplemented!("stub")
            }
            fn blocks(&self) -> &DynBlocks {
                unimplemented!("stub")
            }
            fn known_peers(&self) -> &DynKnownPeers {
                unimplemented!("stub")
            }
            fn current_url(&self) -> Option<Url> {
                None
            }
            fn local_agent_join(&self, _local_agent: DynLocalAgent) -> BoxFut<'_, K2Result<()>> {
                unimplemented!("stub")
            }
            fn local_agent_leave(&self, _local_agent: AgentId) -> BoxFut<'_, ()> {
                unimplemented!("stub")
            }
            fn send_notify(&self, _to_peer: Url, _data: bytes::Bytes) -> BoxFut<'_, K2Result<()>> {
                unimplemented!("stub")
            }
            fn inform_ops_stored(&self, _ops: Vec<StoredOp>) -> BoxFut<'_, K2Result<()>> {
                unimplemented!("stub")
            }
        }

        pub(super) fn stub_space() -> DynSpace {
            Arc::new(StubSpace)
        }
    }

    #[tokio::test]
    async fn service_touch_triggers_ping_and_records_sample() {
        let ping_fn: PingFn =
            Arc::new(|_space, _url| Box::pin(async { Some(Duration::from_millis(42)) }));

        let service = PeerLatencyService::new(ping_fn);
        let space = mock_space::stub_space();
        let url = test_url("ping-me");

        service.touch(url.clone(), space);

        // Give the worker time to process the touch and run pings.
        tokio::time::sleep(Duration::from_millis(500)).await;

        let store = service.store();
        let data = store.lock().expect("latency data lock poisoned");
        assert!(
            data.get_latency(&url).is_some(),
            "expected latency sample to be recorded"
        );
    }

    #[tokio::test]
    async fn service_ping_failing_peer_stops_pinging() {
        let call_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let call_count_fn = Arc::clone(&call_count);

        let ping_fn: PingFn = Arc::new(move |_space, _url| {
            call_count_fn.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Box::pin(async { None }) // Always fail
        });

        let service = PeerLatencyService::new(ping_fn);
        let space = mock_space::stub_space();
        let url = test_url("ping-failing");

        service.touch(url.clone(), space);

        // Wait for initial round of pings + a few worker ticks.
        tokio::time::sleep(Duration::from_millis(500)).await;

        let initial_calls = call_count.load(std::sync::atomic::Ordering::Relaxed);

        // Wait more — if ping-failure detection works, no more pings.
        tokio::time::sleep(Duration::from_millis(500)).await;

        let final_calls = call_count.load(std::sync::atomic::Ordering::Relaxed);

        // Each round bails on the first failing sample, so an always-failing
        // peer should stop being pinged after CONSECUTIVE_FAILURE_THRESHOLD
        // rounds. Stay lenient in case timing lets one extra round slip in.
        assert!(
            final_calls <= initial_calls + (PING_SAMPLE_COUNT as u32),
            "expected pinging to stop, but calls went from {initial_calls} to {final_calls}"
        );
    }

    #[tokio::test]
    async fn ping_round_bails_early_on_first_failure() {
        let call_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let call_count_fn = Arc::clone(&call_count);

        let ping_fn: PingFn = Arc::new(move |_space, _url| {
            call_count_fn.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Box::pin(async { None }) // Always fail on first sample
        });

        let service = PeerLatencyService::new(ping_fn);
        let space = mock_space::stub_space();
        service.touch(test_url("dead"), space);

        // Worker loops without delay until the peer is marked as having failed pings,
        // so 300ms is comfortably enough to exhaust CONSECUTIVE_FAILURE_THRESHOLD.
        tokio::time::sleep(Duration::from_millis(300)).await;

        let count = call_count.load(std::sync::atomic::Ordering::Relaxed);

        // With the early-exit, each round calls ping_fn exactly once before
        // bailing. Without it, each round would have called it
        // PING_SAMPLE_COUNT times.
        assert!(count >= 1, "expected at least one ping attempt");
        assert!(
            count < PING_SAMPLE_COUNT as u32,
            "expected short-circuit: got {count} calls, should be < {PING_SAMPLE_COUNT} per round"
        );
    }

    #[tokio::test]
    async fn ping_round_records_every_successful_sample() {
        let call_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let call_count_fn = Arc::clone(&call_count);

        let ping_fn: PingFn = Arc::new(move |_space, _url| {
            call_count_fn.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Box::pin(async { Some(Duration::from_millis(5)) })
        });

        let service = PeerLatencyService::new(ping_fn);
        let space = mock_space::stub_space();
        let url = test_url("alive");
        service.touch(url.clone(), space);

        tokio::time::sleep(Duration::from_millis(300)).await;

        // On a healthy peer, every sample in the round should fire — no
        // early exit — so the first round alone accounts for
        // PING_SAMPLE_COUNT calls.
        let count = call_count.load(std::sync::atomic::Ordering::Relaxed);
        assert!(
            count >= PING_SAMPLE_COUNT as u32,
            "expected at least {PING_SAMPLE_COUNT} pings for healthy peer, got {count}"
        );
    }

    // -- prune_peer_state_at tests --

    #[test]
    fn prune_keeps_entries_with_live_estimate() {
        let mut peer_state: HashMap<Url, PeerWorkState> = HashMap::new();
        let mut data = LatencyData::default();
        let url = test_url("a");

        // Insert with a last_touched_at far in the past relative to `now`
        // below, so the only reason to keep the entry is the live estimate.
        let touched_at = Instant::now();
        peer_state.insert(
            url.clone(),
            PeerWorkState {
                space: mock_space::stub_space(),
                last_touched_at: touched_at,
            },
        );
        data.record_sample(url.clone(), Duration::from_millis(50));

        let now = touched_at + EVICTION_DURATION + Duration::from_secs(60);
        prune_peer_state_at(&mut peer_state, &mut data, now);
        assert!(peer_state.contains_key(&url));
    }

    #[test]
    fn prune_keeps_recently_touched_entries_without_estimate() {
        let mut peer_state: HashMap<Url, PeerWorkState> = HashMap::new();
        let mut data = LatencyData::default();
        let url = test_url("pending");
        let now = Instant::now();

        // Newly touched peer: no estimate yet (initial ping in flight).
        peer_state.insert(
            url.clone(),
            PeerWorkState {
                space: mock_space::stub_space(),
                last_touched_at: now,
            },
        );

        prune_peer_state_at(&mut peer_state, &mut data, now);
        assert!(peer_state.contains_key(&url));
    }

    #[test]
    fn prune_drops_abandoned_ping_failed_entries() {
        let mut peer_state: HashMap<Url, PeerWorkState> = HashMap::new();
        let mut data = LatencyData::default();
        let url = test_url("dead");
        let now = Instant::now();

        peer_state.insert(
            url.clone(),
            PeerWorkState {
                space: mock_space::stub_space(),
                last_touched_at: now,
            },
        );
        for _ in 0..CONSECUTIVE_FAILURE_THRESHOLD {
            data.record_failure(&url);
        }
        assert!(data.has_failed_pings(&url));

        let future = future_instant(EVICTION_DURATION + Duration::from_secs(60));
        prune_peer_state_at(&mut peer_state, &mut data, future);

        assert!(!peer_state.contains_key(&url));
        // Failure entry dropped alongside peer_state entry.
        assert!(!data.consecutive_failures.contains_key(&url));
        assert!(!data.has_failed_pings(&url));
    }

    #[test]
    fn prune_drops_entries_with_evicted_estimate_and_stale_touch() {
        let mut peer_state: HashMap<Url, PeerWorkState> = HashMap::new();
        let mut data = LatencyData::default();
        let url = test_url("abandoned");
        let now = Instant::now();

        data.record_sample(url.clone(), Duration::from_millis(50));
        peer_state.insert(
            url.clone(),
            PeerWorkState {
                space: mock_space::stub_space(),
                last_touched_at: now,
            },
        );

        let future = future_instant(EVICTION_DURATION + Duration::from_secs(60));
        data.evict_stale_at(future);
        prune_peer_state_at(&mut peer_state, &mut data, future);

        assert!(!peer_state.contains_key(&url));
    }
}
