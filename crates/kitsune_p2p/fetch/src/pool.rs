//! The Fetch Pool: a structure to store ops-to-be-fetched.
//!
//! When we encounter an op hash that we have no record of, we store it as an item
//! at the end of the FetchPool. The items of the queue contain not only the op hash,
//! but also the source(s) to fetch it from, and other data including the last time
//! a fetch was attempted.
//!
//! The consumer of the queue can read items whose last_fetch time is older than some interval
//! from the current moment. The items thus returned are not guaranteed to be returned in
//! order of last_fetch time, but they are guaranteed to be at least as old as the specified
//! interval.

use std::{collections::HashMap, sync::Arc};
use tokio::time::{Duration, Instant};
use indexmap::map::Entry;

use kitsune_p2p_types::{tx2::tx2_utils::ShareOpen, KSpace};

use crate::{FetchContext, FetchKey, FetchPoolPush, RoughInt, FetchSource, SourceState, Sources, queue::MapQueue};

mod pool_reader;
pub use pool_reader::*;

/// A FetchPool tracks a set of [`FetchKey`]s (op hashes) to be fetched,
/// each of which can have multiple sources associated with it.
///
/// When adding the same key twice, the sources are merged by appending the newest
/// source to the front of the list of sources, and the contexts are merged by the
/// method defined in [`FetchPoolConfig`].
///
/// The queue items can be accessed only through its Iterator implementation.
/// Each item contains a FetchKey and one Source agent from which to fetch it.
/// Each time an item is obtained in this way, it is moved to the end of the list.
/// It is important to use the iterator lazily, and only take what is needed.
/// Accessing any item through iteration implies that a fetch was attempted.
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

    /// How long between successive fetches from a particular source, for a particular item?
    /// This protects us from wasting resources on a source which may be offline.
    /// This will eventually be replaced with an exponential backoff which will be
    /// tracked for this source across all items.
    fn source_retry_delay(&self) -> Duration {
        Duration::from_secs(5 * 60)
    }

    /// When a fetch key is added twice, this determines how the two different contexts
    /// get reconciled.
    fn merge_fetch_contexts(&self, a: u32, b: u32) -> u32;
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
pub struct State {
    /// Items ready to be fetched
    queue: MapQueue<FetchKey, FetchPoolItem>,

    /// Fetch sources
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

    /// Get a list of the next items that should be fetched.
    pub fn get_items_to_fetch(&self) -> Vec<(FetchKey, KSpace, FetchSource, Option<FetchContext>)> {
        self.state
            .share_mut(|s| {
                s.poll_items(self.config.clone()).clone()
        })
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

    /// asdf
    pub fn check_sources(&self) {
        self.state.share_mut(|s| {
            s.check_sources();
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

        // Register sources once as they are discovered
        self.sources.entry(source.clone()).or_insert_with(|| {
            SourceState::default()
        });

        match self.queue.entry(key) {
            Entry::Vacant(e) => {
                let sources = Sources::new(
                    [source.clone()],
                );
                let item = FetchPoolItem {
                    sources,
                    space,
                    size,
                    context,
                    last_fetch: None,
                };
                e.insert(item);
            }
            Entry::Occupied(mut e) => {
                let v = e.get_mut();
                v.sources
                    .add(source.clone());
                v.context = match (v.context.take(), context) {
                    (Some(a), Some(b)) => Some(config.merge_fetch_contexts(*a, *b).into()),
                    (Some(a), None) => Some(a),
                    (None, Some(b)) => Some(b),
                    (None, None) => None,
                }
            }
        }
    }

    // TODO update docs
    /// Poll for queue items to fetch. Items accessed will be moved to the end of the queue.
    ///
    /// Only items whose `last_fetch` is more than `interval` ago will be returned.
    pub fn poll_items(&mut self, config: Arc<dyn FetchPoolConfig>) -> Vec<(FetchKey, KSpace, FetchSource, Option<FetchContext>)> {
        let keys: Vec<_> = self
            .queue
            .keys().cloned().collect();

        let batch_size = 100;

        let mut to_fetch = vec![];
        for _ in keys {
            if to_fetch.len() >= batch_size {
                break;
            }

            let (key, item) = match self.queue.front() {
                Some(item) => item,
                None => continue,
            };

            // Check for a pending response for this item
            let should_fetch_item = match &item.last_fetch {
                Some(pending_response) => {
                    if pending_response.when.elapsed() > config.item_retry_delay() {
                        self.sources.get_mut(&pending_response.source).map(|state| {
                            state.record_timeout();
                        });
                        true
                    } else {
                        false
                    }
                }
                None => true
            };

            if should_fetch_item {
                // Clear the last fetch state if it was set. Even if there are no sources and we don't do a fetch, we want to forget
                // the previous request if we're planning to make a new one.
                item.last_fetch = None;

                // Find the next source for this item which is in good standing across other fetches
                if let Some(source) = item.sources.next(|source| {
                    match self.sources.get_mut(source) {
                        Some(state) => {
                            if state.should_use() {
                                return true;
                            }
                        },
                        _ => {
                            tracing::warn!("Not considering source because it is not registered: {:?}", source);
                        },
                    }

                    false
                }) {
                    let space = item.space.clone();
                    item.last_fetch = Some(PendingItemResponse { when: Instant::now(), source: source.clone() });
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
                item.last_fetch.as_ref().map(|pending| {
                    self.sources.get_mut(&pending.source).map(|state| {
                        state.record_response();
                    });
                });
                Some(item)
            }
            None => None,
        }
    }

    /// asdf
    pub fn check_sources(&mut self) {
        self.sources.retain(|_, source| {
            source.check()
        });

        // Drop any sources we are no longer using from the sources used by items
        let keys: Vec<_> = self.queue.keys().cloned().collect();
        for key in keys {
            self.queue.get_mut(&key).expect("Iterating keys").sources.retain(|s| {
                self.sources.contains_key(s)
            });

            // If we've removed all sources from an item, remove the item
            if self.queue.get(&key).expect("Iterating keys").sources.is_empty() {
                self.queue.remove(&key);
            }
        }
    }

    /// Get a string summary of the queue's contents
    #[cfg(any(test, feature = "test_utils"))]
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
                    v.last_fetch
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
    /// The last time we tried fetching this item from any source
    last_fetch: Option<PendingItemResponse>,
}

/// struct
#[derive(Debug, PartialEq, Eq)]
pub struct PendingItemResponse {
    when: Instant,
    source: FetchSource,
}

#[cfg(test)]
mod tests {
    use crate::test_utils::*;
    use arbitrary::Arbitrary;
    use arbitrary::Unstructured;
    use pretty_assertions::assert_eq;
    use std::collections::HashSet;
    use std::{sync::Arc, time::Duration};

    use kitsune_p2p_types::bin_types::{KitsuneBinType, KitsuneSpace};

    use super::*;

    pub(super) struct Config(pub u32, pub u32);

    impl FetchPoolConfig for Config {
        fn item_retry_delay(&self) -> Duration {
            Duration::from_secs(self.0 as u64)
        }

        fn source_retry_delay(&self) -> Duration {
            Duration::from_secs(self.1 as u64)
        }

        fn merge_fetch_contexts(&self, a: u32, b: u32) -> u32 {
            (a + b).min(1)
        }
    }

    pub(super) fn item(
        _cfg: Arc<dyn FetchPoolConfig>,
        sources: Vec<FetchSource>,
        context: Option<FetchContext>,
    ) -> FetchPoolItem {
        FetchPoolItem {
            sources: Sources::new(
                sources
                    .into_iter()
                    .map(|s| (s.clone())),
            ),
            space: Arc::new(KitsuneSpace::new(vec![0; 36])),
            context,
            size: None,
            last_fetch: None,
        }
    }

    fn arbitrary_test_sources(u: &mut Unstructured, count: usize) -> Vec<FetchSource> {
        test_sources(std::iter::repeat_with(|| u8::arbitrary(u).unwrap()).take(count))
    }

    /*
    #[tokio::test(start_paused = true)]
    async fn single_source() {
        let source_delay = Duration::from_secs(10);
        let mut sources = Sources::new(
            [
                test_source(1),
            ]
            .into_iter()
            .collect(),
        );

        assert_eq!(sources.next(source_delay), Some(test_source(1)));

        tokio::time::advance(source_delay).await;

        assert_eq!(sources.next(source_delay), Some(test_source(1)));
        assert_eq!(sources.next(source_delay), None);
    }

    #[tokio::test(start_paused = true)]
    async fn source_rotation() {
        let source_delay = Duration::from_secs(10);
        let mut sources = Sources::new(
            [
                    test_source(1),
                    test_source(2),
            ]
            .into_iter()
            .collect(),
        );

        tokio::time::advance(Duration::from_secs(1)).await;

        assert_eq!(sources.next(source_delay), Some(test_source(2)));
        assert_eq!(sources.next(source_delay), None);

        tokio::time::advance(Duration::from_secs(9)).await;

        assert_eq!(sources.next(source_delay), Some(test_source(1)));

        tokio::time::advance(Duration::from_secs(1)).await;

        assert_eq!(sources.next(source_delay), Some(test_source(2)));
        // source 1 has already had its delay backed off another 10s
        // due to a retry, so it returns None
        assert_eq!(sources.next(source_delay), None);

        tokio::time::advance(Duration::from_secs(10)).await;

        assert_eq!(sources.next(source_delay), Some(test_source(1)));
        assert_eq!(sources.next(source_delay), Some(test_source(2)));
        assert_eq!(sources.next(source_delay), None);
    }

    #[tokio::test(start_paused = true)]
    async fn source_rotation_prioritises_less_recently_tried() {
        let source_delay = Duration::from_secs(10);
        let mut sources = Sources::new(
            [
                test_source(1),
                test_source(2),
                test_source(3),
            ]
            .into_iter()
            .collect(),
        );

        assert_eq!(sources.next(source_delay), Some(test_source(2)));

        // All sources now past their retry delay
        tokio::time::advance(source_delay).await;

        // Would expect source 1 to be tried next but trying 2 first rotated 1 and 2 to the end of the list
        assert_eq!(sources.next(source_delay), Some(test_source(3)));
        assert_eq!(sources.next(source_delay), Some(test_source(1)));
        assert_eq!(sources.next(source_delay), Some(test_source(2)));
        assert_eq!(sources.next(source_delay), None);
    }

    #[tokio::test(start_paused = true)]
    async fn source_rotation_uses_all_sources() {
        let mut noise = [0; 1_000];
        rand::thread_rng().fill_bytes(&mut noise);
        let mut u = Unstructured::new(&noise);

        let source_delay = Duration::from_secs(rand::thread_rng().gen_range(1..50));

        let v = std::iter::repeat_with(|| Duration::arbitrary(&mut u).unwrap())
            .take(100)
            .enumerate()
            .map(|(i, duration)| {
                test_source(i as u8)
            })
            .collect();
        let mut sources = Sources::new(v);

        let mut seen_sources: HashSet<u8> = HashSet::new();
        for _ in 0..100 {
            if let Some(s) = sources.next(source_delay) {
                match s {
                    FetchSource::Agent(a) => {
                        // The source agent key is a repeating byte array of the source number, so we can retrieve any
                        // byte here to get the source number
                        seen_sources.insert(a.0[0]);
                    }
                }
            }

            tokio::time::advance(source_delay).await;
        }

        assert_eq!(100, seen_sources.len());
    }

    #[test]
    fn state_keeps_context_on_merge_if_new_is_none() {
        let mut q = State::default();
        let cfg = Config(1, 1);

        q.push(&cfg, test_req_op(1, test_ctx(1), test_source(1)));
        assert_eq!(test_ctx(1), q.queue.front().unwrap().1.context);

        // Same key but different source so that it will merge and no context set to check how that is merged
        q.push(&cfg, test_req_op(1, None, test_source(0)));
        assert_eq!(test_ctx(1), q.queue.front().unwrap().1.context);
    }

    #[test]
    fn state_adds_context_on_merge_if_current_is_none() {
        let mut q = State::default();
        let cfg = Config(1, 1);

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
        let cfg = Config(1, 1);

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
        let cfg = Config(1, 1);

        q.push(&cfg, test_req_op(1, test_ctx(1), test_source(1)));
        assert_eq!(1, q.queue.front().unwrap().1.sources.len());

        // Set a different context but otherwise the same operation as above
        q.push(&cfg, test_req_op(1, test_ctx(2), test_source(1)));
        assert_eq!(1, q.queue.front().unwrap().1.sources.len());
    }
*/
    #[test]
    fn queue_push() {
        let mut q = State::default();
        let cfg = Arc::new(Config(1, 1));

        // note: new sources get added to the back of the list
        q.push(&*cfg, test_req_op(1, test_ctx(0), test_source(0)));
        q.push(&*cfg, test_req_op(1, test_ctx(1), test_source(1)));

        q.push(&*cfg, test_req_op(2, test_ctx(0), test_source(0)));

        let expected_ready = [
            (test_key_op(1), item(cfg.clone(), test_sources(0..=1), test_ctx(1))),
            (test_key_op(2), item(cfg, test_sources([0]), test_ctx(0))),
        ]
        .into_iter()
        .collect();

        assert_eq!(q.queue, expected_ready);
    }

    #[tokio::test(start_paused = true)]
    async fn queue_next() {
        let cfg = Arc::new(Config(1, 10));
        let mut q = {
            let mut queue = [
                (test_key_op(1), item(cfg.clone(), test_sources(0..=2), test_ctx(1))),
                (test_key_op(2), item(cfg.clone(), test_sources(1..=3), test_ctx(1))),
                (test_key_op(3), item(cfg.clone(), test_sources(2..=4), test_ctx(1))),
            ];
            // Set the last_fetch time of one of the sources to something a bit earlier,
            // so it won't show up in next() right away
            // queue[1]
            //     .1
            //     .sources
            //     .0
            //     .get_mut(&test_source(2))
            //     .unwrap()
            //     .last_request = Some(Instant::now() - Duration::from_secs(3));

            let queue = queue.into_iter().collect();
            State { queue, ..Default::default() }
        };

        // We can try fetching items one source at a time by waiting 1 sec in between

        assert_eq!(3, q.poll_items(cfg.clone()).len());

        tokio::time::advance(Duration::from_secs(1)).await;

        assert_eq!(3, q.poll_items(cfg.clone()).len());

        tokio::time::advance(Duration::from_secs(1)).await;

        assert_eq!(2, q.poll_items(cfg.clone()).len());

        // Wait for manually modified source to be ready
        // (5 + 1 + 1 + 3 = 10)
        tokio::time::advance(Duration::from_secs(5)).await;

        // The next (and only) item will be the one with the timestamp explicitly set
        assert_eq!(
            vec![(test_key_op(2), test_space(0), test_source(2), test_ctx(1))],
            q.poll_items(cfg.clone())
        );
        assert_eq!(0, q.poll_items(cfg.clone()).len());

        // wait long enough for some items to be retryable
        // (10 - 5 - 1 = 4)
        tokio::time::advance(Duration::from_secs(4)).await;

        assert_eq!(3, q.poll_items(cfg).len());
    }

    #[tokio::test(start_paused = true)]
    async fn state_iter_uses_all_sources() {
        let cfg = Arc::new(Config(1, 10));
        let num_items = 10;

        let mut q = {
            let mut queue = vec![];
            for i in 0..num_items {
                queue.push((
                    test_key_op(i as u8),
                    // Give each item a different set of sources
                    item(
                        cfg.clone(),
                        test_sources((i * num_items) as u8..(i * num_items + num_items) as u8),
                        test_ctx(1),
                    ),
                ))
            }

            State {
                queue: queue.into_iter().collect(),
                ..Default::default()
            }
        };

        let mut seen_sources = HashSet::new();
        for _ in 0..num_items {
            q.poll_items(cfg.clone())
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
        let cfg = Arc::new(Config(1, 10));
        let mut q = {
            let queue = [(test_key_op(1), item(cfg.clone(), test_sources([1]), test_ctx(1)))];

            let queue = queue.into_iter().collect();
            State { queue, ..Default::default() }
        };

        assert_eq!(1, q.poll_items(cfg.clone()).len());
        q.remove(&test_key_op(1));

        // Move time forwards to be able to retry the item
        tokio::time::advance(Duration::from_secs(30)).await;

        assert_eq!(0, q.poll_items(cfg).len());
    }

    /*
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
*/
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
