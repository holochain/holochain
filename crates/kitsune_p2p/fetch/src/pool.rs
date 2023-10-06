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

use std::sync::Arc;
use stef::{FileCassette, JsonEncoder};
use tokio::time::{Duration, Instant};

use kitsune_p2p_types::{KAgent, KSpace};
use linked_hash_map::{Entry, LinkedHashMap};

use crate::{FetchContext, FetchKey, FetchPoolPush, RoughInt};

mod pool_reader;
pub use pool_reader::*;

/// Max number of queue items to check on each `next()` poll
const NUM_ITEMS_PER_POLL: usize = 100;

/// Alias
pub type FetchConfig = Arc<dyn FetchPoolConfig>;

/// Host-defined details about how the fetch queue should function
pub trait FetchPoolConfig: 'static + Send + Sync + std::fmt::Debug {
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
#[derive(Debug)]
struct FetchPoolConfigBitwiseOr;

impl FetchPoolConfig for FetchPoolConfigBitwiseOr {
    fn merge_fetch_contexts(&self, a: u32, b: u32) -> u32 {
        a | b
    }
}

/// A FetchPoolState tracks a set of [`FetchKey`]s (op hashes or regions) to be fetched,
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
#[derive(Debug)]
pub struct FetchPoolState {
    config: FetchConfig,
    /// Items ready to be fetched
    queue: LinkedHashMap<FetchKey, FetchPoolItem>,
}

impl Default for FetchPoolState {
    fn default() -> Self {
        Self {
            config: Arc::new(FetchPoolConfigBitwiseOr),
            queue: Default::default(),
        }
    }
}

type NextItem = (FetchKey, KSpace, FetchSource, Option<FetchContext>);

/// Cassette type for recording FetchPool actions
pub type FetchPoolCassette = FileCassette<FetchPoolState, JsonEncoder>;

impl FetchPool {
    /// Constructor
    pub fn new(config: FetchConfig, cassette: Option<FetchPoolCassette>) -> Self {
        Self(stef::Share::new(stef::RecordActions::new(
            cassette,
            FetchPoolState::new(config),
        )))
    }

    /// Constructor, using only the "hardcoded" config (TODO: remove)
    pub fn new_bitwise_or(cassette: Option<FetchPoolCassette>) -> Self {
        let config = Arc::new(FetchPoolConfigBitwiseOr);
        Self::new(config, cassette)
    }

    /// Get a list of the next items that should be fetched.
    pub fn get_items_to_fetch(&self) -> Vec<NextItem> {
        self.write(move |s| std::iter::from_fn(|| s.next_item()).collect())
    }

    /// Get the current size of the fetch pool. This is the number of outstanding items
    /// and may be different to the size of response from `get_items_to_fetch` because it
    /// ignores retry delays.
    pub fn len(&self) -> usize {
        self.read(|s| s.len())
    }

    /// Check whether the fetch pool is empty.
    pub fn is_empty(&self) -> bool {
        self.read(|s| s.queue.is_empty())
    }
}

/// The effect of actions on the FetchPool
#[derive(Debug, PartialEq, Eq, derive_more::From)]
pub enum FetchPoolEffect {
    /// A new item is ready for processing
    NextItem(NextItem),
    /// An item was removed from the pool
    RemovedItem(FetchPoolItem),
}

/// Shared access to FetchPoolState
#[derive(Clone, Debug, derive_more::Deref, stef::State)]
pub struct FetchPool(stef::Share<stef::RecordActions<FetchPoolState>>);

impl From<FetchPoolState> for FetchPool {
    fn from(pool: FetchPoolState) -> Self {
        FetchPool(stef::Share::new(stef::RecordActions::new(Some(()), pool)))
    }
}

#[stef::state(recording, fuzzing, wrapper(FetchPool))]
impl stef::State<'static> for FetchPoolState {
    type Action = FetchPoolAction;
    type Effect = Option<FetchPoolEffect>;

    /// Add an item to the queue.
    /// If the FetchKey does not already exist, add it to the end of the queue.
    /// If the FetchKey exists, add the new source and merge the context in, without
    /// changing the position in the queue.
    #[stef::state( matches( None <=> () ) )]
    #[allow(clippy::unused_unit)]
    fn push(&mut self, args: FetchPoolPush) -> () {
        let FetchPoolPush {
            key,
            author,
            context,
            space,
            source,
            size,
            cause: _,
        } = args;

        match self.queue.entry(key) {
            Entry::Vacant(e) => {
                let sources = if let Some(author) = author {
                    // TODO This is currently not used. The idea is that the author will always be a valid alternative to fetch
                    //      this data from. See one of the call sites for `push` to see where this was intended to be used from.
                    Sources(
                        [
                            (source.clone(), SourceRecord::new(source)),
                            (
                                FetchSource::Agent(author.clone()),
                                SourceRecord::agent(author),
                            ),
                        ]
                        .into_iter()
                        .collect(),
                    )
                } else {
                    Sources(
                        [(source.clone(), SourceRecord::new(source))]
                            .into_iter()
                            .collect(),
                    )
                };
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
                    .0
                    .insert(source.clone(), SourceRecord::new(source));
                v.context = match (v.context.take(), context) {
                    (Some(a), Some(b)) => Some(self.config.merge_fetch_contexts(*a, *b).into()),
                    (Some(a), None) => Some(a),
                    (None, Some(b)) => Some(b),
                    (None, None) => None,
                }
            }
        }
    }

    /// Get the next item to be fetched
    #[stef::state(
        matches(
            Some(FetchPoolEffect::NextItem(i)) <=> Some(i),
            None <=> None
        )
    )]
    fn next_item(&mut self) -> Option<NextItem> {
        let keys: Vec<_> = self
            .queue
            .keys()
            .take(NUM_ITEMS_PER_POLL)
            .cloned()
            .collect();

        for key in keys {
            let item = self.queue.get_refresh(&key)?;
            let item_not_recently_fetched = item
                .last_fetch
                .map(|t| t.elapsed() >= self.config.item_retry_delay())
                .unwrap_or(true); // true on the first fetch before `last_fetch` is set
            if item_not_recently_fetched {
                if let Some(source) = item.sources.next(self.config.source_retry_delay()) {
                    // TODO what if we're recently tried to use this source and it's not available? The retry delay does not apply across items
                    let space = item.space.clone();
                    item.last_fetch = Some(Instant::now());
                    return Some((key, space, source, item.context));
                }
            }
        }

        None
    }

    /// When an item has been successfully fetched, we can remove it from the queue.
    #[stef::state(
        matches(
            Some(FetchPoolEffect::RemovedItem(i)) <=> Some(i),
            None <=> None
        )
    )]
    fn remove(&mut self, key: FetchKey) -> Option<FetchPoolItem> {
        self.queue.remove(&key)
    }
}

impl FetchPoolState {
    /// Constructor
    pub fn new(config: FetchConfig) -> Self {
        Self {
            config,
            queue: Default::default(),
        }
    }

    /// Get the current size of the fetch pool. This is the number of outstanding items
    /// and may be different to the size of response from `get_items_to_fetch` because it
    /// ignores retry delays.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Is the queue empty?
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
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
                    v.sources.0.len(),
                    v.last_fetch
                        .map(|t| format!("{:?}", t.elapsed()))
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
    last_fetch: Option<Instant>,
}

#[derive(Debug, PartialEq, Eq)]
struct SourceRecord {
    source: FetchSource,
    last_request: Option<Instant>,
}

impl SourceRecord {
    fn new(source: FetchSource) -> Self {
        Self {
            source,
            last_request: None,
        }
    }

    fn agent(agent: KAgent) -> Self {
        Self::new(FetchSource::Agent(agent))
    }
}

/// Fetch item within the fetch queue state.
#[derive(Debug, PartialEq, Eq)]
struct Sources(LinkedHashMap<FetchSource, SourceRecord>);

impl Sources {
    fn next(&mut self, interval: Duration) -> Option<FetchSource> {
        let source_keys: Vec<FetchSource> = self.0.keys().cloned().collect();
        for source in source_keys {
            if let Some(sr) = self.0.get_refresh(&source) {
                if sr
                    .last_request
                    .map(|t| t.elapsed() >= interval)
                    .unwrap_or(true)
                {
                    sr.last_request = Some(Instant::now());
                    return Some(source);
                }
            }
        }

        None
    }
}

/// A source to fetch from: either a node, or an agent on a node
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub enum FetchSource {
    /// An agent on a node
    Agent(KAgent),
}

#[cfg(test)]
mod tests {
    use crate::test_utils::*;
    use crate::FetchCause;
    use arbitrary::Arbitrary;
    use arbitrary::Unstructured;
    use pretty_assertions::assert_eq;
    use rand::{Rng, RngCore};
    use std::collections::HashSet;
    use std::{sync::Arc, time::Duration};

    use kitsune_p2p_types::bin_types::{KitsuneBinType, KitsuneSpace};

    use super::*;

    #[derive(Debug)]
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

    pub(super) fn item(sources: Vec<FetchSource>, context: Option<FetchContext>) -> FetchPoolItem {
        FetchPoolItem {
            sources: Sources(
                sources
                    .into_iter()
                    .map(|s| (s.clone(), SourceRecord::new(s)))
                    .collect(),
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

    #[tokio::test(start_paused = true)]
    async fn single_source() {
        let source_delay = Duration::from_secs(10);
        let mut sources = Sources(
            [(
                test_source(1),
                SourceRecord {
                    source: test_source(1),
                    last_request: None,
                },
            )]
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
        let mut sources = Sources(
            [
                (
                    test_source(1),
                    SourceRecord {
                        source: test_source(1),
                        last_request: Some(Instant::now()),
                    },
                ),
                (
                    test_source(2),
                    SourceRecord {
                        source: test_source(2),
                        last_request: None,
                    },
                ),
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
        let mut sources = Sources(
            [
                (
                    test_source(1),
                    SourceRecord {
                        source: test_source(1),
                        last_request: Some(Instant::now()), // recently tried
                    },
                ),
                (
                    test_source(2),
                    SourceRecord {
                        source: test_source(2),
                        last_request: None, // never checked
                    },
                ),
                (
                    test_source(3),
                    SourceRecord {
                        source: test_source(3),
                        last_request: None, // never checked
                    },
                ),
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
                (
                    test_source(i as u8),
                    SourceRecord {
                        source: test_source(i as u8),
                        last_request: Instant::now().checked_sub(duration),
                    },
                )
            })
            .collect();
        let mut sources = Sources(v);

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
        let mut q = FetchPoolState::new(Arc::new(Config(1, 1)));

        q.push(test_req_op(1, test_ctx(1), test_source(1)));
        assert_eq!(test_ctx(1), q.queue.front().unwrap().1.context);

        // Same key but different source so that it will merge and no context set to check how that is merged
        q.push(test_req_op(1, None, test_source(0)));
        assert_eq!(test_ctx(1), q.queue.front().unwrap().1.context);
    }

    #[test]
    fn state_adds_context_on_merge_if_current_is_none() {
        let mut q = FetchPoolState::new(Arc::new(Config(1, 1)));

        // Initially have no context
        q.push(test_req_op(1, None, test_source(1)));
        assert_eq!(None, q.queue.front().unwrap().1.context);

        // Now merge with a context
        q.push(test_req_op(1, test_ctx(1), test_source(0)));
        assert_eq!(test_ctx(1), q.queue.front().unwrap().1.context);
    }

    #[test]
    fn state_can_merge_two_items_without_contexts() {
        let mut q = FetchPoolState::new(Arc::new(Config(1, 1)));

        // Initially have no context
        q.push(test_req_op(1, None, test_source(1)));
        assert_eq!(None, q.queue.front().unwrap().1.context);

        // Now merge with no context
        q.push(test_req_op(1, None, test_source(0)));

        // Still no context
        assert_eq!(None, q.queue.front().unwrap().1.context);
        // but both sources are present
        assert_eq!(2, q.queue.front().unwrap().1.sources.0.len());
    }

    #[test]
    fn state_ignores_duplicate_sources_on_merge() {
        let mut q = FetchPoolState::new(Arc::new(Config(1, 1)));

        q.push(test_req_op(1, test_ctx(1), test_source(1)));
        assert_eq!(1, q.queue.front().unwrap().1.sources.0.len());

        // Set a different context but otherwise the same operation as above
        q.push(test_req_op(1, test_ctx(2), test_source(1)));
        assert_eq!(1, q.queue.front().unwrap().1.sources.0.len());
    }

    #[test]
    fn queue_push() {
        let mut q = FetchPoolState::new(Arc::new(Config(1, 1)));

        // note: new sources get added to the back of the list
        q.push(test_req_op(1, test_ctx(0), test_source(0)));
        q.push(test_req_op(1, test_ctx(1), test_source(1)));

        q.push(test_req_op(2, test_ctx(0), test_source(0)));

        let expected_ready = [
            (test_key_op(1), item(test_sources(0..=1), test_ctx(1))),
            (test_key_op(2), item(test_sources([0]), test_ctx(0))),
        ]
        .into_iter()
        .collect();

        assert_eq!(q.queue, expected_ready);
    }

    #[tokio::test(start_paused = true)]
    async fn queue_next() {
        let cfg = Config(1, 10);
        let pool: FetchPool = {
            let mut queue = [
                (test_key_op(1), item(test_sources(0..=2), test_ctx(1))),
                (test_key_op(2), item(test_sources(1..=3), test_ctx(1))),
                (test_key_op(3), item(test_sources(2..=4), test_ctx(1))),
            ];
            // Set the last_fetch time of one of the sources to something a bit earlier,
            // so it won't show up in next() right away
            queue[1]
                .1
                .sources
                .0
                .get_mut(&test_source(2))
                .unwrap()
                .last_request = Some(Instant::now() - Duration::from_secs(3));

            let queue = queue.into_iter().collect();
            FetchPoolState {
                queue,
                config: Arc::new(cfg),
            }
            .into()
        };

        // We can try fetching items one source at a time by waiting 1 sec in between

        assert_eq!(pool.get_items_to_fetch().len(), 3);

        tokio::time::advance(Duration::from_secs(1)).await;

        assert_eq!(pool.get_items_to_fetch().len(), 3);

        tokio::time::advance(Duration::from_secs(1)).await;

        assert_eq!(pool.get_items_to_fetch().len(), 2);

        // Wait for manually modified source to be ready
        // (5 + 1 + 1 + 3 = 10)
        tokio::time::advance(Duration::from_secs(5)).await;

        // The next (and only) item will be the one with the timestamp explicitly set
        assert_eq!(
            std::iter::from_fn(|| pool.write(|p| p.next_item())).collect::<Vec<_>>(),
            vec![(test_key_op(2), test_space(0), test_source(2), test_ctx(1))]
        );
        assert_eq!(pool.get_items_to_fetch().len(), 0);

        // wait long enough for some items to be retryable
        // (10 - 5 - 1 = 4)
        tokio::time::advance(Duration::from_secs(4)).await;

        assert_eq!(pool.get_items_to_fetch().len(), 3);
    }

    #[tokio::test(start_paused = true)]
    async fn state_get_items_to_fetch_sees_all_items() {
        let cfg = Config(1, 10);
        let num_items = 2 * NUM_ITEMS_PER_POLL; // Must be greater than NUM_ITEMS_PER_POLL

        let q: FetchPool = {
            let mut queue = vec![];
            for i in 0..(num_items) {
                queue.push((
                    test_key_op(i as u8),
                    item(test_sources([(i % 100) as u8]), test_ctx(1)),
                ))
            }

            FetchPoolState {
                queue: queue.into_iter().collect(),
                config: Arc::new(cfg),
            }
            .into()
        };

        // None fetched initially, should see all items
        assert_eq!(num_items, q.len());

        assert_eq!(q.get_items_to_fetch().len(), num_items);

        // Everything seen, no time elapsed
        assert_eq!(q.get_items_to_fetch().len(), 0);

        // Move time forwards so everything will be ready to retry
        tokio::time::advance(Duration::from_secs(30)).await;

        assert_eq!(q.get_items_to_fetch().len(), num_items);
    }

    #[tokio::test(start_paused = true)]
    async fn state_iter_uses_all_sources() {
        let cfg = Config(1, 10);
        let num_items = 10;

        let mut q = {
            let mut queue = vec![];
            for i in 0..num_items {
                queue.push((
                    test_key_op(i as u8),
                    // Give each item a different set of sources
                    item(
                        test_sources((i * num_items) as u8..(i * num_items + num_items) as u8),
                        test_ctx(1),
                    ),
                ))
            }

            FetchPoolState {
                queue: queue.into_iter().collect(),
                config: Arc::new(cfg),
            }
        };

        let mut seen_sources = HashSet::new();
        for _ in 0..num_items {
            std::iter::from_fn(|| q.next_item())
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
        let cfg = Config(1, 10);
        let mut pool: FetchPool = {
            let queue = [(test_key_op(1), item(test_sources([1]), test_ctx(1)))];

            let queue = queue.into_iter().collect();
            FetchPoolState {
                queue,
                config: Arc::new(cfg),
            }
            .into()
        };

        assert_eq!(1, pool.get_items_to_fetch().len());
        pool.remove(test_key_op(1));

        // Move time forwards to be able to retry the item
        tokio::time::advance(Duration::from_secs(30)).await;

        assert_eq!(0, pool.get_items_to_fetch().len());
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_pool() {
        // Use a nearly real fetch config.
        #[derive(Debug)]
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
        let mut fetch_pool = FetchPool::new(Arc::new(TestFetchConfig {}), None);

        // Some sources will be unavailable for blocks of time
        let unavailable_sources: HashSet<FetchSource> =
            arbitrary_test_sources(&mut u, 10).into_iter().collect();

        // Add one item that will never send
        fetch_pool.push(FetchPoolPush {
            key: test_key_op(220),
            space: test_space(u8::arbitrary(&mut u).unwrap()),
            source: unavailable_sources.iter().last().cloned().unwrap(),
            size: None,   // Not important for this test
            author: None, // Unused field, ignore
            context: test_ctx(u32::arbitrary(&mut u).unwrap()),
            cause: FetchCause::Publish,
        });

        let mut failed_count = 0;
        for i in (0..200).step_by(5) {
            // Add five items to fetch
            for _ in 0..5 {
                fetch_pool.push(FetchPoolPush {
                    key: test_key_op(i),
                    space: test_space(u8::arbitrary(&mut u).unwrap()),
                    source: test_source(u8::arbitrary(&mut u).unwrap()),
                    size: None,   // Not important for this test
                    author: None, // Unused field, ignore
                    context: test_ctx(u32::arbitrary(&mut u).unwrap()),
                    cause: FetchCause::Publish,
                });
            }

            // Try to process all items (because that's how this is used in practice)
            let items = fetch_pool.get_items_to_fetch();
            for item in items {
                // If the source is available the fetch succeeds and we remove the item, otherwise leave it in the pool
                if !unavailable_sources.contains(&item.2) {
                    fetch_pool.remove(item.0);
                } else {
                    failed_count += 1;
                }
            }

            // Advance time to allow items retry with a difference source if necessary
            tokio::time::advance(fetch_pool.read(|s| s.config.item_retry_delay())).await;
        }

        // We created an item that will always fail, so should have at least one left
        assert!(
            !fetch_pool.get_items_to_fetch().is_empty(),
            "Pool should have had at least one item but got \n {}",
            fetch_pool.read(|s| format!("{}\n{}", FetchPoolState::summary_heading(), s.summary()))
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
    fn default_fetch_context_merge_maintains_flags_from_both_contexts() {
        const FLAG_1: u32 = 1 << 5;
        const FLAG_2: u32 = 1 << 10;

        let context_1 = FetchContext(FLAG_1);
        let context_2 = FetchContext(FLAG_2);

        let pool = FetchPool::new_bitwise_or(None);
        let merged = pool.read(|s| s.config.merge_fetch_contexts(*context_1, *context_2));

        assert_eq!(FLAG_1, merged & FLAG_1);
        assert_eq!(FLAG_2, merged & FLAG_2);
        assert_eq!(0, merged ^ (FLAG_1 | FLAG_2)); // Clear FLAG_1 and FLAG_2 to check no other bits are set
    }

    #[test_strategy::proptest]
    #[cfg(feature = "fuzzing")]
    fn fuzz_fetchpool_drainage(spaces: [KSpace; 3], actions: Vec<FetchPoolAction>) {
        use stef::State;

        // println!("-------------------------------");

        #[derive(Debug)]
        struct Config;

        impl FetchPoolConfig for Config {
            fn item_retry_delay(&self) -> Duration {
                Duration::from_millis(10)
            }

            fn source_retry_delay(&self) -> Duration {
                Duration::from_millis(10)
            }

            fn merge_fetch_contexts(&self, a: u32, b: u32) -> u32 {
                a | b
            }
        }

        let mut pool = FetchPoolState::new(Arc::new(Config));

        // apply the random actions
        for (i, mut a) in actions.into_iter().enumerate() {
            match a {
                FetchPoolAction::Push(ref mut push) => {
                    // set all actions to one of 3 spaces
                    push.space = spaces[i % 3].clone();
                }
                _ => (),
            }
            pool.transition(a);
        }

        // println!("{}", pool.summary());

        // drain the pool via next_item
        while pool.len() > 0 {
            if let Some((key, _, _, _)) = pool.next_item() {
                pool.remove(key);
            } else {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        let reader = FetchPoolReader::from(FetchPool::from(pool));
        let info = reader.info(spaces[0..3].iter().cloned().collect());
        assert_eq!(info.num_ops_to_fetch, 0);
        assert_eq!(info.op_bytes_to_fetch, 0);
    }
}
