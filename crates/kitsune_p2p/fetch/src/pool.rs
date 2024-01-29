//! The Fetch Pool: a structure to store ops-to-be-fetched.
//!
//! When we encounter an op hash that we have no record of, we store it as an item
//! at the end of the FetchPool. The items of the queue contain not only the op hash,
//! but also the source(s) to fetch it from, and other data including the last time
//! a fetch was attempted.
//!
//! The consumer of the queue can read items whose last fetch time is older than some interval
//! from the current moment. The items thus returned are not guaranteed to be returned in
//! order of last fetch time, but they are guaranteed to be at least as old as the specified
//! interval.

use indexmap::map::Entry;
use std::{collections::HashMap, sync::Arc};
use tokio::time::{Duration, Instant};

use kitsune_p2p_types::{tx2::tx2_utils::ShareOpen, KSpace};

use crate::{
    queue::MapQueue,
    source::{FetchSource, SourceState, Sources},
    FetchContext, FetchKey, FetchPoolPush, RoughInt,
};

mod pool_reader;
pub use pool_reader::*;

/// A FetchPool tracks a set of [`FetchKey`]s (op hashes) to be fetched,
/// each of which can have multiple sources associated with it.
///
/// When adding the same key twice, the sources are merged by appending the newest
/// source to the front of the list of sources, and the contexts are merged by the
/// method defined in [`FetchPoolConfig`].
///
/// Each item consists of a FetchKey (Op) and one or more sources (Agent) from which to fetch it.
/// Items can be retrieved in batches using [`FetchPool::get_items_to_fetch`]. Any items which
/// were considered while building the batch, either because they were still awaiting a response
/// or because they were returned in the batch, will be moved to the end of the queue. This makes
/// fetching items reasonably fair.
#[derive(Clone)]
pub struct FetchPool {
    config: FetchConfig,
    state: ShareOpen<State>,
}

impl std::fmt::Debug for FetchPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.state
            .share_ref(|state| f.debug_struct("FetchPool").field("state", state).finish())
    }
}

/// Alias
pub type FetchConfig = Arc<dyn FetchPoolConfig>;

/// Host-defined details about how the fetch queue should function
pub trait FetchPoolConfig: 'static + Send + Sync {
    /// How long between successive item fetches, regardless of source?
    /// This gives a source a fair chance to respond before proceeding with a
    /// different source.
    ///
    /// The most conservative setting for this is `2 * tuning_params.implicit_timeout`,
    /// since that is the maximum amount of time a successful response can take.
    /// Lower values will give up early and may result in duplicate data sent if the
    /// response takes a long time to come back.
    fn item_retry_delay(&self) -> Duration {
        Duration::from_secs(90)
    }

    /// How long to put a source on a backoff if it fails to respond to a fetch.
    /// This is an initial value for a backoff on the source and will be increased if the source remains unresponsive.
    ///
    /// With the default settings of 30s for this delay and 8 retries, the total retry period is around 20 minutes (with jitter) so that the
    /// time we keep sources in the pool is close to the default value for the TTL on agent info. This means if an agent goes offline then
    /// they should be removed from the fetch pool in a similar amount of time to other communication with them ceasing.
    fn source_retry_delay(&self) -> Duration {
        Duration::from_secs(30)
    }

    /// When a fetch key is added twice, this determines how the two different contexts
    /// get reconciled.
    fn merge_fetch_contexts(&self, a: u32, b: u32) -> u32;

    /// How many items should be returned for fetching per call to [`FetchPool::get_items_to_fetch`].
    fn fetch_batch_size(&self) -> usize {
        100
    }

    /// The number of times a source can fail to respond in time before it is put on a backoff.
    ///
    /// This is a total number of timeouts so if a source is unreliable over time then it will be put on a backoff even if it is currently responding.
    /// If the source responds after its timeout period then this counter will be reset and the source will be considered available again after
    /// a single backoff period.
    ///
    /// The reasoning behind this parameter is that we want to limit the amount of resources we sink into an unresponsive source,
    /// as well as limiting the load on the source itself, who may be unresponsive because they're already struggling with too much load.
    fn source_unavailable_timeout_threshold(&self) -> usize {
        30
    }
}

// TODO: move this to host, but for now, for convenience, we just use this one config
//       for every queue
struct FetchPoolConfigBitwiseOr;

impl FetchPoolConfig for FetchPoolConfigBitwiseOr {
    fn merge_fetch_contexts(&self, a: u32, b: u32) -> u32 {
        a | b
    }
}

/// The actual inner state of the FetchPool, from which items can be obtained
#[derive(Debug, Default)]
pub(crate) struct State {
    /// Items to be fetched, ordered by least recently considered for fetching.
    queue: MapQueue<FetchKey, FetchPoolItem>,

    /// The state of all sources that we have seen in [`FetchPoolPush`]es.
    ///
    /// Note that sources are put on a backoff if they fail to respond to enough fetches. If the backoff
    /// expires and the source is still not responding, it will be removed from this map.
    sources: HashMap<FetchSource, SourceState>,
}

impl FetchPool {
    /// Constructor
    pub fn new(config: FetchConfig) -> Self {
        Self {
            config,
            state: ShareOpen::new(State::default()),
        }
    }

    /// Constructor, using only the "hardcoded" config (TODO: remove)
    pub fn new_bitwise_or() -> Self {
        Self {
            config: Arc::new(FetchPoolConfigBitwiseOr),
            state: ShareOpen::new(State::default()),
        }
    }

    /// Add an item to the queue.
    /// If the FetchKey does not already exist, add it to the end of the queue.
    /// If the FetchKey exists, add the new source and merge the context in, without
    /// changing the position in the queue.
    pub fn push(&self, args: FetchPoolPush) {
        self.state.share_mut(|s| {
            tracing::debug!(
                "FetchPool (size = {}) item added: {:?}",
                s.queue.len() + 1,
                args
            );
            s.push(&*self.config, args);
        });
    }

    /// Check if an item is in the fetch pool and what its context is.
    pub fn check_item(&self, key: &FetchKey) -> (bool, Option<FetchContext>) {
        self.state.share_ref(|s| match s.queue.get(key) {
            Some(item) => (true, item.context),
            None => (false, None),
        })
    }

    /// When an item has been successfully fetched, we can remove it from the queue.
    pub fn remove(&self, key: &FetchKey) -> Option<FetchPoolItem> {
        self.state.share_mut(|s| {
            let removed = s.remove(key);
            tracing::debug!(
                "FetchPool (size = {}) item removed: key={:?} val={:?}",
                s.queue.len(),
                key,
                removed
            );
            removed
        })
    }

    /// Get a list of the next items to be fetched.
    pub fn get_items_to_fetch(&self) -> Vec<(FetchKey, KSpace, FetchSource, Option<FetchContext>)> {
        self.state
            .share_mut(|s| s.get_batch(self.config.clone()).clone())
    }

    /// Get the current size of the fetch pool. This is the number of outstanding items
    /// and may be different to the size of response from `get_items_to_fetch` because it
    /// ignores retry delays.
    pub fn len(&self) -> usize {
        self.state.share_ref(|s| s.queue.len())
    }

    /// Check whether the fetch pool is empty.
    pub fn is_empty(&self) -> bool {
        self.state.share_ref(|s| s.queue.is_empty())
    }

    /// Check the state of all sources and remove any that have expired. See the docs on State::check_sources for details.
    pub fn check_sources(&self) {
        self.state.share_mut(|s| {
            s.check_sources(self.config.clone());
        });
    }
}

impl State {
    /// Add an item to the queue.
    /// If the FetchKey does not already exist, add it to the end of the queue.
    /// If the FetchKey exists, add the new source and merge the context in, without
    /// changing the position in the queue.
    pub fn push(&mut self, config: &dyn FetchPoolConfig, args: FetchPoolPush) {
        let FetchPoolPush {
            key,
            context,
            space,
            source,
            size,
            ..
        } = args;

        // Register sources once as they are discovered, with a default initial state
        self.sources.entry(source.clone()).or_default();

        match self.queue.entry(key) {
            Entry::Vacant(e) => {
                let sources = Sources::new([source.clone()]);
                let item = FetchPoolItem {
                    sources,
                    space,
                    size,
                    context,
                    pending_response: None,
                };
                e.insert(item);
            }
            Entry::Occupied(mut e) => {
                let v = e.get_mut();
                v.sources.add(source.clone());
                v.context = match (v.context.take(), context) {
                    (Some(a), Some(b)) => Some(config.merge_fetch_contexts(*a, *b).into()),
                    (Some(a), None) => Some(a),
                    (None, Some(b)) => Some(b),
                    (None, None) => None,
                }
            }
        }
    }

    /// Poll for a batch of queue items to fetch. The size of the batch is determined by [`FetchPoolConfig::fetch_batch_size`].
    /// Items which are accessed while trying to fill the batch will be moved to the end of the queue. This is the case
    /// even if the item was not returned in the batch because it was waiting for a response already.
    pub fn get_batch(
        &mut self,
        config: Arc<dyn FetchPoolConfig>,
    ) -> Vec<(FetchKey, KSpace, FetchSource, Option<FetchContext>)> {
        let batch_size = config.fetch_batch_size();

        let mut to_fetch = vec![];
        // The queue provides a `front` method which will repeatedly loop over all the items it contains to bound the
        // search by the size of the queue.
        for _ in 0..self.queue.len() {
            // If we have enough items, stop looking
            if to_fetch.len() >= batch_size {
                break;
            }

            // Get the next item from the queue
            let (key, item) = match self.queue.front() {
                Some(item) => item,
                None => continue,
            };

            // Check for a pending response on this item
            let should_fetch_item = match &item.pending_response {
                Some(pending_response) => {
                    if pending_response.when.elapsed() > config.item_retry_delay() {
                        if let Some(state) = self.sources.get_mut(&pending_response.source) {
                            state.record_timeout();
                        }
                        true
                    } else {
                        false
                    }
                }
                None => true,
            };

            if should_fetch_item {
                // Clear the last fetch state if it was set. Even if there are no sources and we don't do a fetch, we want to forget
                // the previous request if we're planning to make a new one.
                item.pending_response = None;

                // Find the next source for this item which is in good standing across other fetches
                if let Some(source) = item.sources.next(|source| {
                    match self.sources.get_mut(source) {
                        Some(state) => {
                            if state.should_use() {
                                return true;
                            }
                        }
                        _ => {
                            tracing::warn!(
                                "Not considering source because it is not registered: {:?}",
                                source
                            );
                        }
                    }

                    false
                }) {
                    let space = item.space.clone();
                    item.pending_response = Some(PendingItemResponse {
                        when: Instant::now(),
                        source: source.clone(),
                    });
                    to_fetch.push((key.clone(), space, source, item.context));
                }
            }
        }

        to_fetch
    }

    /// When an item has been successfully fetched, we can remove it from the queue.
    pub fn remove(&mut self, key: &FetchKey) -> Option<FetchPoolItem> {
        match self.queue.remove(key) {
            Some(item) => {
                if let Some(pending) = item.pending_response.as_ref() {
                    if let Some(state) = self.sources.get_mut(&pending.source) {
                        state.record_response();
                    }
                }

                Some(item)
            }
            None => None,
        }
    }

    /// Check for sources which have expired and remove them from the list of sources.
    /// Any ops which don't have any sources left will be removed from the queue.
    pub fn check_sources(&mut self, config: FetchConfig) {
        self.sources
            .retain(|_, source| source.is_valid(config.clone()));

        // Drop any sources we are no longer using from the sources used by items
        let keys: Vec<_> = self.queue.keys().cloned().collect();
        for key in keys {
            self.queue
                .get_mut(&key)
                .expect("Iterating keys")
                .sources
                .retain(|s| self.sources.contains_key(s));

            // If we've removed all sources from an item, remove the item
            if self
                .queue
                .get(&key)
                .expect("Iterating keys")
                .sources
                .is_empty()
            {
                self.queue.remove(&key);
            }
        }
    }

    /// Get a string summary of the queue's contents
    #[cfg(any(test, feature = "test_utils"))]
    #[allow(dead_code)]
    pub fn summary(&self) -> String {
        use human_repr::HumanCount;

        let table = self
            .queue
            .iter()
            .map(|(k, v)| {
                let key = match k {
                    FetchKey::Op(hash) => {
                        let h = hash.to_string();
                        format!("{}..{}", &h[0..4], &h[h.len() - 4..])
                    }
                };

                let size = v.size.unwrap_or_default().get();
                format!(
                    "{:10}  {:^6} {:^6} {:>6}",
                    key,
                    v.sources.len(),
                    v.pending_response
                        .as_ref()
                        .map(|t| format!("{:?}", t.when.elapsed()))
                        .unwrap_or_else(|| "-".to_string()),
                    size.human_count_bytes(),
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("{}\n{} items total", table, self.queue.len())
    }

    /// The heading to go along with the summary
    #[cfg(any(test, feature = "test_utils"))]
    #[allow(dead_code)]
    pub fn summary_heading() -> String {
        format!("{:10}  {:>6} {:>6} {}", "key", "#src", "last", "size")
    }
}

/// An item in the queue, corresponding to a single op or region to fetch
#[derive(Debug, PartialEq, Eq)]
pub struct FetchPoolItem {
    /// Known sources from whom we can fetch this item.
    /// Sources will always be tried in order.
    sources: Sources,
    /// The space to retrieve this op from
    space: KSpace,
    /// Approximate size of the item. If set, the item will be counted towards overall progress.
    size: Option<RoughInt>,
    /// Opaque user data specified by the host
    pub context: Option<FetchContext>,
    /// If there is a response pending for this item then track the source and when the request was made.
    pending_response: Option<PendingItemResponse>,
}

/// Tracks the source and when a request was made for a [`FetchPoolItem`]. This is used to track timeouts
/// for sources that don't respond before the configured timeout.
#[derive(Debug, PartialEq, Eq)]
pub struct PendingItemResponse {
    when: Instant,
    source: FetchSource,
}

#[cfg(test)]
mod tests {
    use crate::backoff::BACKOFF_RETRY_COUNT;
    use crate::test_utils::*;
    use crate::TransferMethod;
    use arbitrary::Arbitrary;
    use arbitrary::Unstructured;
    use pretty_assertions::assert_eq;
    use rand::RngCore;
    use std::collections::HashSet;
    use std::{sync::Arc, time::Duration};

    use kitsune_p2p_types::bin_types::{KitsuneBinType, KitsuneSpace};

    use super::*;

    pub(super) fn item(
        _cfg: Arc<dyn FetchPoolConfig>,
        sources: Vec<FetchSource>,
        context: Option<FetchContext>,
    ) -> FetchPoolItem {
        FetchPoolItem {
            sources: Sources::new(sources.into_iter()),
            space: Arc::new(KitsuneSpace::new(vec![0; 36])),
            context,
            size: None,
            pending_response: None,
        }
    }

    fn arbitrary_test_sources(u: &mut Unstructured, count: usize) -> Vec<FetchSource> {
        test_sources(std::iter::repeat_with(|| u8::arbitrary(u).unwrap()).take(count))
    }

    #[test]
    fn state_keeps_context_on_merge_if_new_is_none() {
        let mut q = State::default();
        let cfg = TestFetchConfig(1, 1);

        q.push(&cfg, test_req_op(1, test_ctx(1), test_source(1)));
        assert_eq!(test_ctx(1), q.queue.front().unwrap().1.context);

        // Same key but different source so that it will merge and no context set to check how that is merged
        q.push(&cfg, test_req_op(1, None, test_source(0)));
        assert_eq!(test_ctx(1), q.queue.front().unwrap().1.context);
    }

    #[test]
    fn state_adds_context_on_merge_if_current_is_none() {
        let mut q = State::default();
        let cfg = TestFetchConfig(1, 1);

        // Initially have no context
        q.push(&cfg, test_req_op(1, None, test_source(1)));
        assert_eq!(None, q.queue.front().unwrap().1.context);

        // Now merge with a context
        q.push(&cfg, test_req_op(1, test_ctx(1), test_source(0)));
        assert_eq!(test_ctx(1), q.queue.front().unwrap().1.context);
    }

    #[test]
    fn state_can_merge_two_items_without_contexts() {
        let mut q = State::default();
        let cfg = TestFetchConfig(1, 1);

        // Initially have no context
        q.push(&cfg, test_req_op(1, None, test_source(1)));
        assert_eq!(None, q.queue.front().unwrap().1.context);

        // Now merge with no context
        q.push(&cfg, test_req_op(1, None, test_source(0)));

        // Still no context
        assert_eq!(None, q.queue.front().unwrap().1.context);
        // but both sources are present
        assert_eq!(2, q.queue.front().unwrap().1.sources.len());
    }

    #[test]
    fn state_ignores_duplicate_sources_on_merge() {
        let mut q = State::default();
        let cfg = TestFetchConfig(1, 1);

        q.push(&cfg, test_req_op(1, test_ctx(1), test_source(1)));
        assert_eq!(1, q.queue.front().unwrap().1.sources.len());

        // Set a different context but otherwise the same operation as above
        q.push(&cfg, test_req_op(1, test_ctx(2), test_source(1)));
        assert_eq!(1, q.queue.front().unwrap().1.sources.len());
    }

    #[test]
    fn queue_push() {
        let mut q = State::default();
        let cfg = Arc::new(TestFetchConfig(1, 1));

        // note: new sources get added to the back of the list
        q.push(&*cfg, test_req_op(1, test_ctx(0), test_source(0)));
        q.push(&*cfg, test_req_op(1, test_ctx(1), test_source(1)));

        q.push(&*cfg, test_req_op(2, test_ctx(0), test_source(0)));

        let expected_ready = [
            (
                test_key_op(1),
                item(cfg.clone(), test_sources(0..=1), test_ctx(1)),
            ),
            (test_key_op(2), item(cfg, test_sources([0]), test_ctx(0))),
        ]
        .into_iter()
        .collect();

        assert_eq!(q.queue, expected_ready);
    }

    #[tokio::test(start_paused = true)]
    async fn queue_next() {
        let cfg = Arc::new(TestFetchConfig(5, 10));
        let mut q = {
            let mut queue = [
                (
                    test_key_op(1),
                    item(cfg.clone(), test_sources(0..=2), test_ctx(1)),
                ),
                (
                    test_key_op(2),
                    item(cfg.clone(), test_sources(1..=3), test_ctx(1)),
                ),
                (
                    test_key_op(3),
                    item(cfg.clone(), test_sources(2..=4), test_ctx(1)),
                ),
            ];

            queue[1].1.pending_response = Some(PendingItemResponse {
                when: Instant::now() - Duration::from_secs(3),
                source: test_source(1),
            });

            let queue = queue.into_iter().collect();
            State {
                queue,
                sources: test_sources(0..=4)
                    .into_iter()
                    .map(|s| (s, SourceState::default()))
                    .collect(),
            }
        };

        // We can try fetching items one source at a time by waiting 1 sec in between

        assert_eq!(2, q.get_batch(cfg.clone()).len());

        tokio::time::advance(Duration::from_secs(3)).await;

        assert_eq!(1, q.get_batch(cfg.clone()).len());

        tokio::time::advance(Duration::from_secs(10)).await;

        assert_eq!(3, q.get_batch(cfg.clone()).len());
    }

    #[tokio::test(start_paused = true)]
    async fn uses_all_sources() {
        let cfg = Arc::new(TestFetchConfig(1, 10));
        let num_items = 10;

        let mut q = {
            let mut queue = vec![];
            let mut sources = vec![];
            for i in 0..num_items {
                let these_sources =
                    test_sources((i * num_items) as u8..(i * num_items + num_items) as u8);
                queue.push((
                    test_key_op(i as u8),
                    // Give each item a different set of sources
                    item(cfg.clone(), these_sources.clone(), test_ctx(1)),
                ));

                sources.extend(these_sources);
            }

            State {
                queue: queue.into_iter().collect(),
                sources: sources
                    .into_iter()
                    .map(|s| (s, SourceState::default()))
                    .collect(),
            }
        };

        let mut seen_sources = HashSet::new();
        for _ in 0..num_items {
            q.get_batch(cfg.clone())
                .into_iter()
                .map(|item| match item.2 {
                    FetchSource::Agent(a) => a.0.clone(),
                })
                .for_each(|source| {
                    seen_sources.insert(source);
                });

            // Move time forwards so everything will be ready to retry
            tokio::time::advance(Duration::from_secs(30)).await;
        }

        assert_eq!(num_items * num_items, seen_sources.len());
    }

    #[tokio::test(start_paused = true)]
    async fn remove_fetch_item() {
        holochain_trace::test_run().unwrap();

        let cfg = Arc::new(TestFetchConfig(1, 10));
        let mut q: State = {
            let queue = [(
                test_key_op(1),
                item(cfg.clone(), test_sources([1]), test_ctx(1)),
            )];
            let queue = queue.into_iter().collect();

            let sources = [(test_source(1), SourceState::default())]
                .into_iter()
                .collect();
            State { queue, sources }
        };

        assert_eq!(1, q.get_batch(cfg.clone()).len());
        q.remove(&test_key_op(1));

        // Move time forwards to be able to retry the item
        tokio::time::advance(Duration::from_secs(30)).await;

        assert_eq!(0, q.get_batch(cfg).len());
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_pool() {
        // Use a nearly real fetch config.
        struct TestFetchConfig {}
        impl FetchPoolConfig for TestFetchConfig {
            // Don't really care about this, but the default trait functions for timeouts are wanted for this test
            fn merge_fetch_contexts(&self, a: u32, b: u32) -> u32 {
                a | b
            }
        }

        // Prepare random inputs
        let mut noise = [0; 1_000];
        rand::thread_rng().fill_bytes(&mut noise);
        let mut u = Unstructured::new(&noise);

        // Create a fetch pool to test
        let fetch_pool = FetchPool::new(Arc::new(TestFetchConfig {}));

        // Some sources will be unavailable for blocks of time
        let unavailable_sources: HashSet<FetchSource> =
            arbitrary_test_sources(&mut u, 10).into_iter().collect();

        // Add one item that will never send
        fetch_pool.push(FetchPoolPush {
            key: test_key_op(220),
            space: test_space(u8::arbitrary(&mut u).unwrap()),
            source: unavailable_sources.iter().last().cloned().unwrap(),
            size: None, // Not important for this test
            context: test_ctx(u32::arbitrary(&mut u).unwrap()),
            transfer_method: TransferMethod::Gossip,
        });

        let mut failed_count = 0;
        for i in (0..200).step_by(5) {
            // Add five items to fetch
            for _ in 0..5 {
                fetch_pool.push(FetchPoolPush {
                    key: test_key_op(i),
                    space: test_space(u8::arbitrary(&mut u).unwrap()),
                    source: test_source(u8::arbitrary(&mut u).unwrap()),
                    size: None, // Not important for this test
                    context: test_ctx(u32::arbitrary(&mut u).unwrap()),
                    transfer_method: TransferMethod::Gossip,
                });
            }

            // Try to process all items (because that's how this is used in practice)
            let items = fetch_pool.get_items_to_fetch();
            for item in items {
                // If the source is available the fetch succeeds and we remove the item, otherwise leave it in the pool
                if !unavailable_sources.contains(&item.2) {
                    fetch_pool.remove(&item.0);
                } else {
                    failed_count += 1;
                }
            }

            // Advance time to allow items retry with a difference source if necessary
            tokio::time::advance(fetch_pool.config.item_retry_delay()).await;
        }

        // We created an item that will always fail, so should have at least one left
        assert!(
            !fetch_pool.get_items_to_fetch().is_empty(),
            "Pool should have had at least one item but got \n {}",
            fetch_pool.state.share_ref(|s| format!(
                "{}\n{}",
                State::summary_heading(),
                s.summary()
            ))
        );

        // 10 accounted for by the item we've set up to never succeed, possible to get more but not guaranteed to not
        // asserting
        assert!(
            failed_count >= 10,
            "At least 10 items should have failed to be fetched but was {}",
            failed_count
        );
    }

    #[test]
    fn check_item_missing() {
        let fetch_pool = FetchPool::new(Arc::new(TestFetchConfig(1, 1)));
        assert_eq!((false, None), fetch_pool.check_item(&test_key_op(1)));
    }

    #[test]
    fn drain_fetch_pool() {
        // Use a nearly real fetch config.
        struct TestFetchConfig {}
        impl FetchPoolConfig for TestFetchConfig {
            // Don't really care about this, but the default trait functions for timeouts are wanted for this test
            fn merge_fetch_contexts(&self, a: u32, b: u32) -> u32 {
                a | b
            }
        }

        // Create a fetch pool to test
        let fetch_pool = FetchPool::new(Arc::new(TestFetchConfig {}));

        for i in (0..200).step_by(5) {
            for j in 0..5 {
                fetch_pool.push(FetchPoolPush {
                    key: test_key_op(i),
                    space: test_space(j),
                    source: test_source(j),
                    size: None, // Not important for this test
                    context: test_ctx(0),
                    transfer_method: TransferMethod::Gossip,
                });
            }
        }

        for _ in 0..2 {
            for (key, _, _, _) in fetch_pool.get_items_to_fetch() {
                if fetch_pool.check_item(&key).0 {
                    fetch_pool.remove(&key);
                }
            }
        }

        assert!(fetch_pool.is_empty());
        assert_eq!(0, fetch_pool.get_items_to_fetch().len());
    }

    #[tokio::test(start_paused = true)]
    async fn drop_expired_sources() {
        let config = Arc::new(TestFetchConfig(1, 1));
        let fetch_pool = FetchPool::new(config.clone());

        // First op with one source
        fetch_pool.push(FetchPoolPush {
            key: test_key_op(1),
            space: test_space(1),
            source: test_source(1),
            size: None,
            context: test_ctx(0),
            transfer_method: TransferMethod::Gossip,
        });

        // Second op with two sources
        fetch_pool.push(FetchPoolPush {
            key: test_key_op(2),
            space: test_space(1),
            source: test_source(1),
            size: None,
            context: test_ctx(0),
            transfer_method: TransferMethod::Gossip,
        });

        // Add the second source to the op above
        fetch_pool.push(FetchPoolPush {
            key: test_key_op(2),
            space: test_space(1),
            source: test_source(2),
            size: None,
            context: test_ctx(0),
            transfer_method: TransferMethod::Gossip,
        });

        // Send enough ops for the first source to be put on a backoff
        for _ in 0..(config.source_unavailable_timeout_threshold() + 1) {
            fetch_pool.get_items_to_fetch();

            // Wait long enough for items to be retried
            tokio::time::advance(2 * config.item_retry_delay()).await;
        }

        // Check sources to mark the first source on a backoff
        fetch_pool.check_sources();

        for _ in 0..BACKOFF_RETRY_COUNT {
            // Need to wait by both the source and item retry delays, accounting for source delays being increased in the backoff
            tokio::time::advance(1000 * config.source_retry_delay()).await;

            assert_eq!(2, fetch_pool.get_items_to_fetch().len());
        }

        let keep_source_two_alive_key = test_key_op(5);
        fetch_pool.push(FetchPoolPush {
            key: keep_source_two_alive_key.clone(),
            space: test_space(1),
            source: test_source(2),
            size: None,
            context: test_ctx(0),
            transfer_method: TransferMethod::Gossip,
        });

        // Verify the item is in the pool and remove it again to mark a successful fetch for source 2
        assert!(fetch_pool.check_item(&keep_source_two_alive_key).0);
        fetch_pool.remove(&keep_source_two_alive_key);

        // Now check sources to remove source 1 which hasn't had a successful receive in the backoff period
        fetch_pool.check_sources();

        // Should have dropped source 1 from the pool, which means op 1 is gone and op 2 should only have 1 source
        assert_eq!(1, fetch_pool.len());

        // Wait for the first item to be ready again
        tokio::time::advance(2 * config.item_retry_delay()).await;

        let batch = fetch_pool.get_items_to_fetch();
        assert_eq!(1, batch.len());
        assert_eq!(test_source(2), batch.first().unwrap().2);
    }

    #[test]
    fn default_fetch_context_merge_maintains_flags_from_both_contexts() {
        const FLAG_1: u32 = 1 << 5;
        const FLAG_2: u32 = 1 << 10;

        let context_1 = FetchContext(FLAG_1);
        let context_2 = FetchContext(FLAG_2);

        let pool = FetchPool::new_bitwise_or();
        let merged = pool.config.merge_fetch_contexts(*context_1, *context_2);

        assert_eq!(FLAG_1, merged & FLAG_1);
        assert_eq!(FLAG_2, merged & FLAG_2);
        assert_eq!(0, merged ^ (FLAG_1 | FLAG_2)); // Clear FLAG_1 and FLAG_2 to check no other bits are set
    }
}
