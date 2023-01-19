//! The Fetch Queue: a structure to store ops-to-be-fetched.
//!
//! When we encounter an op hash that we have no record of, we store it as an item
//! at the end of the FetchQueue. The items of the queue contain not only the op hash,
//! but also the source(s) to fetch it from, and other data including the last time
//! a fetch was attempted.
//!
//! The consumer of the queue can read items whose last_fetch time is older than some interval
//! from the current moment. The items thus returned are not guaranteed to be returned in
//! order of last_fetch time, but they are guaranteed to be at least as old as the specified
//! interval.

use std::{collections::HashMap, sync::Arc};
use tokio::time::{Duration, Instant};

use kitsune_p2p_types::{tx2::tx2_utils::ShareOpen, KAgent, KSpace /*, Tx2Cert*/};
use linked_hash_map::{Entry, LinkedHashMap};

use crate::{FetchContext, FetchKey, FetchQueuePush, RoughInt};

mod queue_reader;
pub use queue_reader::*;

/// Max number of queue items to check on each `next()` poll
const NUM_ITEMS_PER_POLL: usize = 100;

/// A FetchQueue tracks a set of [`FetchKey`]s (op hashes or regions) to be fetched,
/// each of which can have multiple sources associated with it.
///
/// When adding the same key twice, the sources are merged by appending the newest
/// source to the front of the list of sources, and the contexts are merged by the
/// method defined in [`FetchQueueConfig`].
///
/// The queue items can be accessed only through its Iterator implementation.
/// Each item contains a FetchKey and one Source agent from which to fetch it.
/// Each time an item is obtained in this way, it is moved to the end of the list.
/// It is important to use the iterator lazily, and only take what is needed.
/// Accessing any item through iteration implies that a fetch was attempted.
#[derive(Clone)]
pub struct FetchQueue {
    config: FetchConfig,
    state: ShareOpen<State>,
}

impl std::fmt::Debug for FetchQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.state
            .share_ref(|state| f.debug_struct("FetchQueue").field("state", state).finish())
    }
}

/// Alias
pub type FetchConfig = Arc<dyn FetchQueueConfig>;

/// Host-defined details about how the fetch queue should function
pub trait FetchQueueConfig: 'static + Send + Sync {
    /// How long between successive item fetches, regardless of source?
    /// This gives a source a fair chance to respond before proceeding with a
    /// different source.
    ///
    /// The most conservative setting for this is `2 * tuning_params.implicit_timeout`,
    /// since that is the maximum amount of time a successful response can take.
    /// Lower values will give up early and may result in duplicate data sent if the
    /// response takes a long time to come back.
    fn item_retry_delay(&self) -> std::time::Duration {
        std::time::Duration::from_secs(90)
    }

    /// How long between successive fetches from a particular source, for a particular item?
    /// This protects us from wasting resources on a source which may be offline.
    /// This will eventually be replaced with an exponential backoff which will be
    /// tracked for this source across all items.
    fn source_retry_delay(&self) -> std::time::Duration {
        std::time::Duration::from_secs(5 * 60)
    }

    /// When a fetch key is added twice, this determines how the two different contexts
    /// get reconciled.
    fn merge_fetch_contexts(&self, a: u32, b: u32) -> u32;
}

/// The actual inner state of the FetchQueue, from which items can be obtained
#[derive(Debug)]
pub struct State {
    /// Items ready to be fetched
    queue: LinkedHashMap<FetchKey, FetchQueueItem>,
    /// The list of sources used throughout the lifetime of the FetchQueue
    sources: HashMap<FetchSource, SharedSource>,
}

#[allow(clippy::derivable_impls)]
impl Default for State {
    fn default() -> Self {
        Self {
            queue: Default::default(),
            sources: Default::default(),
        }
    }
}

/// A mutable iterator over the FetchQueue State
pub struct StateIter<'a> {
    state: &'a mut State,
    config: &'a dyn FetchQueueConfig,
}

type SharedSource = ShareOpen<SourceRecord>;

/// Fetch item within the fetch queue state.
#[derive(Debug, Default, PartialEq, Eq)]
struct Sources(Vec<SharedSource>);

impl Sources {
    // /// Create new shareable list from sources. These sources have not been shared
    // /// anywhere else.
    // #[cfg(test)]
    // fn from_new(sources: impl IntoIterator<Item = FetchSource>) -> Self {
    //     Self(
    //         sources
    //             .into_iter()
    //             .map(|s| ShareOpen::new(SourceRecord::new(s)))
    //             .collect(),
    //     )
    // }

    /// Create new shareable list from records. These sources have not been shared
    /// anywhere else.
    #[cfg(test)]
    fn from_records(sources: impl IntoIterator<Item = SourceRecord>) -> Self {
        Self(sources.into_iter().map(|s| ShareOpen::new(s)).collect())
    }
}

/// An item in the queue, corresponding to a single op or region to fetch
#[derive(Debug, PartialEq, Eq)]
pub struct FetchQueueItem {
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

/// A source to fetch from: either a node, or an agent on a node
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FetchSource {
    /// An agent on a node
    Agent(KAgent),
}

// TODO: move this to host, but for now, for convenience, we just use this one config
// for every queue
struct FetchQueueConfigBitwiseOr;

impl FetchQueueConfig for FetchQueueConfigBitwiseOr {
    fn merge_fetch_contexts(&self, a: u32, b: u32) -> u32 {
        a | b
    }
}

impl FetchQueue {
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
            config: Arc::new(FetchQueueConfigBitwiseOr),
            state: ShareOpen::new(State::default()),
        }
    }

    /// Add an item to the queue.
    /// If the FetchKey does not already exist, add it to the end of the queue.
    /// If the FetchKey exists, add the new source and merge the context in, without
    /// changing the position in the queue.
    pub fn push(&self, args: FetchQueuePush) {
        self.state.share_mut(|s| {
            tracing::debug!(
                "FetchQueue (size = {}) item added: {:?}",
                s.queue.len() + 1,
                args
            );
            s.push(&*self.config, args);
        });
    }

    /// When an item has been successfully fetched, we can remove it from the queue.
    pub fn remove(&self, key: &FetchKey) -> Option<FetchQueueItem> {
        self.state.share_mut(|s| {
            let removed = s.remove(key);
            tracing::debug!(
                "FetchQueue (size = {}) item removed: key={:?} val={:?}",
                s.queue.len(),
                key,
                removed
            );
            removed
        })
    }

    /// Get a list of the next items that should be fetched.
    pub fn get_items_to_fetch(&self) -> Vec<(FetchKey, KSpace, FetchSource, Option<FetchContext>)> {
        self.state.share_mut(|s| {
            let mut out = Vec::new();

            for (key, space, source, context) in s.iter_mut(&*self.config) {
                out.push((key, space, source, context));
            }

            out
        })
    }
}

impl State {
    /// Add an item to the queue.
    /// If the FetchKey does not already exist, add it to the end of the queue.
    /// If the FetchKey exists, add the new source and merge the context in, without
    /// changing the position in the queue.
    pub fn push(&mut self, config: &dyn FetchQueueConfig, args: FetchQueuePush) {
        let FetchQueuePush {
            key,
            author,
            context,
            space,
            source,
            size,
        } = args;

        let sources = if let Some(author) = author {
            Sources(self.shared_sources([source, FetchSource::Agent(author)]))
        } else {
            Sources(self.shared_sources([source]))
        };

        match self.queue.entry(key) {
            Entry::Vacant(e) => {
                let item = FetchQueueItem {
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
                for source in sources.0.into_iter().rev() {
                    v.sources.0.insert(0, source);
                }
                v.context = match (v.context.take(), context) {
                    (Some(a), Some(b)) => Some(config.merge_fetch_contexts(*a, *b).into()),
                    (a, b) => a.and(b),
                }
            }
        }
    }

    /// Access queue items through mutable iteration. Items accessed will be moved
    /// to the end of the queue.
    ///
    /// Only items whose `last_fetch` is more than `interval` ago will be returned.
    pub fn iter_mut<'a>(&'a mut self, config: &'a dyn FetchQueueConfig) -> StateIter {
        StateIter {
            state: self,
            config,
        }
    }

    /// When an item has been successfully fetched, we can remove it from the queue.
    pub fn remove(&mut self, key: &FetchKey) -> Option<FetchQueueItem> {
        self.queue.remove(key)
    }

    fn shared_sources(&self, sources: impl IntoIterator<Item = FetchSource>) -> Vec<SharedSource> {
        sources
            .into_iter()
            .map(|s| {
                self.sources
                    .get(&s)
                    .cloned()
                    .unwrap_or_else(|| ShareOpen::new(SourceRecord::new(s)))
            })
            .collect()
    }
}

impl<'a> Iterator for StateIter<'a> {
    type Item = (FetchKey, KSpace, FetchSource, Option<FetchContext>);

    fn next(&mut self) -> Option<Self::Item> {
        let keys: Vec<_> = self
            .state
            .queue
            .keys()
            .take(NUM_ITEMS_PER_POLL)
            .cloned()
            .collect();
        for key in keys {
            let item = self.state.queue.get_refresh(&key)?;
            let item_not_recently_fetched = item
                .last_fetch
                .map(|t| t.elapsed() >= self.config.item_retry_delay())
                .unwrap_or(true);
            if item_not_recently_fetched {
                if let Some(source) = item.sources.next(self.config.source_retry_delay()) {
                    let space = item.space.clone();
                    item.last_fetch = Some(Instant::now());
                    return Some((key, space, source, item.context));
                }
            }
        }
        None
    }
}

impl SourceRecord {
    fn new(source: FetchSource) -> Self {
        Self {
            source,
            last_request: None,
        }
    }

    /// True if this source requested more than `interval` ago
    /// or it was never requested.
    fn is_ready(&self, interval: Duration) -> bool {
        self.last_request
            .map(|t| t.elapsed() >= interval)
            .unwrap_or(true)
    }
}

impl Sources {
    fn next(&mut self, interval: Duration) -> Option<FetchSource> {
        if let Some((i, agent)) =
            self.0.iter_mut().enumerate().find_map(|(i, s)| {
                s.share_ref(|s| s.is_ready(interval).then(|| (i, s.source.clone())))
            })
        {
            self.0[i].share_mut(|s| {
                s.last_request = Some(Instant::now());
            });
            self.0.rotate_left(i + 1);
            Some(agent)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {

    use pretty_assertions::assert_eq;
    use std::{sync::Arc, time::Duration};

    use kitsune_p2p_types::bin_types::{KitsuneAgent, KitsuneBinType, KitsuneOpHash, KitsuneSpace};

    use super::*;

    #[derive(Clone, Debug)]
    pub struct Config(pub u32, pub u32);

    pub struct TestSources {
        _config: Config,
        sources: Sources,
    }

    impl TestSources {
        pub fn new(_config: Config) -> Self {
            Self {
                _config,
                sources: Sources(vec![]),
            }
        }

        pub fn item(
            &mut self,
            indices: impl IntoIterator<Item = u8>,
            context: Option<FetchContext>,
        ) -> FetchQueueItem {
            let sources = Sources(
                indices
                    .into_iter()
                    .map(|i| {
                        if let Some(s) = self.sources.0.get(i as usize).cloned() {
                            s
                        } else {
                            let s = ShareOpen::new(SourceRecord::new(source(i)));
                            self.sources.0.insert(i.into(), s.clone());
                            s
                        }
                    })
                    .collect(),
            );
            FetchQueueItem {
                sources,
                space: Arc::new(KitsuneSpace::new(vec![0; 36])),
                context,
                size: None,
                last_fetch: None,
            }
        }

        pub(super) fn into_hashmap(self) -> HashMap<FetchSource, SharedSource> {
            self.sources
                .0
                .into_iter()
                .enumerate()
                .map(|(i, s)| (source(i as u8), s))
                .collect()
        }
    }

    impl FetchQueueConfig for Config {
        fn merge_fetch_contexts(&self, a: u32, b: u32) -> u32 {
            (a + b).min(1)
        }

        fn item_retry_delay(&self) -> Duration {
            Duration::from_secs(self.0 as u64)
        }

        fn source_retry_delay(&self) -> Duration {
            Duration::from_secs(self.1 as u64)
        }
    }

    pub(super) fn key_op(n: u8) -> FetchKey {
        FetchKey::Op(Arc::new(KitsuneOpHash::new(vec![n; 36])))
    }

    pub(super) fn req(n: u8, context: Option<FetchContext>, source: FetchSource) -> FetchQueuePush {
        FetchQueuePush {
            key: key_op(n),
            author: None,
            context,
            space: space(0),
            source,
            size: None,
        }
    }

    pub(super) fn space(i: u8) -> KSpace {
        Arc::new(KitsuneSpace::new(vec![i; 36]))
    }

    pub(super) fn source(i: u8) -> FetchSource {
        FetchSource::Agent(Arc::new(KitsuneAgent::new(vec![i; 36])))
    }

    pub(super) fn ctx(c: u32) -> Option<FetchContext> {
        Some(c.into())
    }

    #[tokio::test(start_paused = true)]
    async fn source_rotation() {
        let sec1 = Duration::from_secs(10);
        let mut ss = Sources::from_records(vec![
            SourceRecord {
                source: source(1),
                last_request: Some(Instant::now()),
            }
            .into(),
            SourceRecord {
                source: source(2),
                last_request: None,
            }
            .into(),
        ]);

        tokio::time::advance(Duration::from_secs(1)).await;

        assert_eq!(ss.next(sec1), Some(source(2)));
        assert_eq!(ss.next(sec1), None);

        tokio::time::advance(Duration::from_secs(9)).await;

        assert_eq!(ss.next(sec1), Some(source(1)));

        tokio::time::advance(Duration::from_secs(1)).await;

        assert_eq!(ss.next(sec1), Some(source(2)));
        // source 1 has already had its delay backed off to 20s
        // due to a retry, so it returns None
        assert_eq!(ss.next(sec1), None);

        tokio::time::advance(Duration::from_secs(20)).await;

        assert_eq!(ss.next(sec1), Some(source(1)));
        assert_eq!(ss.next(sec1), Some(source(2)));
        assert_eq!(ss.next(sec1), None);
    }

    #[test]
    fn queue_push() {
        let mut q = State::default();
        let cfg = Config(1, 1);
        let mut ss = TestSources::new(cfg.clone());

        // note: new sources get added to the front of the list
        q.push(&cfg, req(1, ctx(1), source(1)));
        q.push(&cfg, req(1, ctx(0), source(0)));

        q.push(&cfg, req(2, ctx(0), source(0)));

        let expected_ready = [
            (key_op(1), ss.item(0..=1, ctx(1))),
            (key_op(2), ss.item([0], ctx(0))),
        ]
        .into_iter()
        .collect();

        assert_eq!(q.queue, expected_ready);
    }

    #[tokio::test(start_paused = true)]
    async fn queue_next() {
        let cfg = Config(1, 10);
        let mut ss = TestSources::new(cfg.clone());
        let mut q = {
            let queue = [
                (key_op(1), ss.item(0..=2, ctx(1))),
                (key_op(2), ss.item(1..=3, ctx(1))),
                (key_op(3), ss.item(2..=4, ctx(1))),
            ];
            // Set the last_fetch time of one of the sources to something a bit earlier,
            // so it won't show up in next() right away
            queue[1].1.sources.0[1]
                .share_mut(|s| s.last_request = Some(Instant::now() - Duration::from_secs(3)));

            let queue = queue.into_iter().collect();
            let sources = ss.into_hashmap();
            State { queue, sources }
        };

        // We can try fetching items one source at a time by waiting 1 sec in between

        assert_eq!(q.iter_mut(&cfg).count(), 3);

        tokio::time::advance(Duration::from_secs(1)).await;

        assert_eq!(q.iter_mut(&cfg).count(), 3);

        tokio::time::advance(Duration::from_secs(1)).await;

        assert_eq!(q.iter_mut(&cfg).count(), 2);

        // Wait for manually modified source to be ready
        // (5 + 1 + 1 + 3 = 10)
        tokio::time::advance(Duration::from_secs(5)).await;

        // The next (and only) item will be the one with the timestamp explicitly set
        assert_eq!(
            q.iter_mut(&cfg).collect::<Vec<_>>(),
            vec![(key_op(2), space(0), source(2), ctx(1))]
        );
        assert_eq!(q.iter_mut(&cfg).count(), 0);

        // wait long enough for some items to be retryable
        // (10 - 5 - 1 = 4)
        tokio::time::advance(Duration::from_secs(4)).await;

        assert_eq!(q.iter_mut(&cfg).count(), 3);
    }

    #[tokio::test(start_paused = true)]
    async fn queue_expiry() {}
}
